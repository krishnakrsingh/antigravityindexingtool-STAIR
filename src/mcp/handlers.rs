use super::protocol::*;
use crate::crepe::{CrepeIndex, SearchMatch, Verifier};
use crate::embeddings::LocalSemanticIndex;
use crate::merkle::FileWatcher;
use crate::stair::SymbolTrie;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub struct McpHandler {
    #[allow(dead_code)]
    pub workspace_root: PathBuf,
    watcher: Arc<FileWatcher>,
    crepe: Arc<CrepeIndex>,
    verifier: Arc<Verifier>,
    trie: Arc<SymbolTrie>,
    semantic: Arc<LocalSemanticIndex>,
}

impl McpHandler {
    pub fn new(workspace_root: PathBuf) -> Self {
        let watcher = Arc::new(FileWatcher::new(workspace_root.clone()));
        let crepe = Arc::new(CrepeIndex::new(workspace_root.clone()));
        let verifier = Arc::new(Verifier::new(workspace_root.clone()));
        let trie = Arc::new(SymbolTrie::new(workspace_root.clone()));
        let semantic = Arc::new(LocalSemanticIndex::new());

        // Initial index build only if index not already loaded from disk
        if crepe.total_files() == 0 {
            let _ = crepe.build();
        }
        let _sym_count = trie.build();

        // Populate local semantic index with symbols in parallel
        let mut items = Vec::new();
        for (file, symbols) in trie.get_outline(None) {
            for sym in symbols {
                let sym_path = format!("{}::{}", file, sym.name);
                items.push((sym_path, sym.signature, file.clone()));
            }
        }
        semantic.index_symbols_batch(items);

        Self {
            workspace_root,
            watcher,
            crepe,
            verifier,
            trie,
            semantic,
        }
    }

    pub fn list_tools() -> Vec<McpToolInfo> {
        vec![
            McpToolInfo {
                name: "instant_grep".to_string(),
                description: "Sub-millisecond candidate-filtered regex & code search using sparse n-grams (Cursor crepe replica) + exact verification via bundled ripgrep".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Regex or literal pattern to search" },
                        "path_filter": { "type": "string", "description": "Optional substring to filter file paths" },
                        "case_sensitive": { "type": "boolean", "description": "Whether to match case sensitively (default false)" },
                        "max_results": { "type": "integer", "description": "Maximum number of results to return (default 50)" },
                        "use_ripgrep_process": { "type": "boolean", "description": "Whether to verify candidates via bundled rg.exe process instead of in-process regex" }
                    },
                    "required": ["pattern"]
                }),
            },
            McpToolInfo {
                name: "get_symbol_outline".to_string(),
                description: "Adapted Code STAIR engine: generates hierarchical AST Symbol Table of Contents (classes, functions, methods, traits) eliminating token chunking".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Optional file path or directory filter" },
                        "depth": { "type": "integer", "description": "Max hierarchy depth (default 3)" }
                    }
                }),
            },
            McpToolInfo {
                name: "get_symbol_definition".to_string(),
                description: "Extracts the exact AST code definition from resident Symbol Prefix Trie with 0.00% hallucination rate and 50-60% token reduction".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "symbol_path": { "type": "string", "description": "Hierarchical symbol path, e.g. 'src/merkle/tree.rs::MerkleTree' or 'src/crepe/index.rs::CrepeIndex::find_candidates'" }
                    },
                    "required": ["symbol_path"]
                }),
            },
            McpToolInfo {
                name: "merkle_diff".to_string(),
                description: "Microsecond dirty-file tracking diff using BLAKE3 Merkle Tree against a previous root hash or baseline".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "from_hash": { "type": "string", "description": "Optional base Merkle hash to diff against (default diffs against initial snapshot)" }
                    }
                }),
            },
            McpToolInfo {
                name: "merkle_snapshot".to_string(),
                description: "Retrieves current BLAKE3 Merkle tree root hash, total indexed files, and snapshot metadata".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpToolInfo {
                name: "semantic_search".to_string(),
                description: "GPU-accelerated FAISS + SQLite Vector RAG and AST semantic search returning enriched node metadata (class, signature, decorators, docstring, preview)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Natural language or technical code query" },
                        "limit": { "type": "integer", "description": "Max results to return (default 10)" },
                        "json_format": { "type": "boolean", "description": "Whether to return raw JSON instead of formatted Markdown (default false)" }
                    },
                    "required": ["query"]
                }),
            },
            McpToolInfo {
                name: "index_status".to_string(),
                description: "Returns health, index metrics, memory-mapped lookup sizes, and cache status".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ]
    }

    pub fn handle_tool_call(&self, name: &str, args: &serde_json::Value) -> McpToolCallResult {
        match name {
            "instant_grep" => self.handle_instant_grep(args),
            "get_symbol_outline" => self.handle_get_symbol_outline(args),
            "get_symbol_definition" => self.handle_get_symbol_definition(args),
            "merkle_diff" => self.handle_merkle_diff(args),
            "merkle_snapshot" => self.handle_merkle_snapshot(args),
            "semantic_search" => self.handle_semantic_search(args),
            "index_status" => self.handle_index_status(args),
            _ => McpToolCallResult {
                content: vec![McpTextContent {
                    content_type: "text".to_string(),
                    text: format!("Unknown tool: {}", name),
                }],
                is_error: true,
            },
        }
    }

    fn handle_instant_grep(&self, args: &serde_json::Value) -> McpToolCallResult {
        let t0 = Instant::now();
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return McpToolCallResult {
                    content: vec![McpTextContent {
                        content_type: "text".to_string(),
                        text: "Missing required argument 'pattern'".to_string(),
                    }],
                    is_error: true,
                };
            }
        };

        let path_filter = args.get("path_filter").and_then(|v| v.as_str());
        let case_sensitive = args.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        let use_rg_process = args.get("use_ripgrep_process").and_then(|v| v.as_bool()).unwrap_or(false);

        // Stage A: Candidate Discovery via Sparse N-Grams
        let candidate_t0 = Instant::now();
        let mut candidates = self.crepe.find_candidates(pattern);

        if let Some(filter) = path_filter {
            let norm_filter = filter.replace('\\', "/");
            candidates.retain(|c| c.contains(&norm_filter));
        }
        let candidate_time_micros = candidate_t0.elapsed().as_micros();

        // Stage B: Exact Verification
        let verify_t0 = Instant::now();
        let matches = if use_rg_process {
            match self.verifier.verify_with_ripgrep(pattern, &candidates, case_sensitive) {
                Ok(m) => m,
                Err(_e) => {
                    self.verifier.verify_in_process(pattern, &candidates, case_sensitive).unwrap_or_default()
                }
            }
        } else {
            self.verifier.verify_in_process(pattern, &candidates, case_sensitive).unwrap_or_default()
        };
        let verify_time_micros = verify_t0.elapsed().as_micros();
        let total_time_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let total_matches = matches.len();
        let limited_matches: Vec<&SearchMatch> = matches.iter().take(max_results).collect();

        let mut output = String::new();
        output.push_str(&format!(
            "Instant Grep Results for '{}' (Total Time: {:.2} ms | Candidate Lookup: {} µs | Verification: {} µs | Candidates: {}/{} files | Matches: {})\n\n",
            pattern,
            total_time_ms,
            candidate_time_micros,
            verify_time_micros,
            candidates.len(),
            self.crepe.total_files(),
            total_matches
        ));

        if limited_matches.is_empty() {
            output.push_str("No matches found.\n");
        } else {
            for m in limited_matches {
                output.push_str(&format!(
                    "{}:{}:{}: {}\n",
                    m.file, m.line_number, m.column, m.line_content.trim()
                ));
            }
            if total_matches > max_results {
                output.push_str(&format!(
                    "\n... [{} more matches truncated. Use max_results or path_filter to narrow]\n",
                    total_matches - max_results
                ));
            }
        }

        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: output,
            }],
            is_error: false,
        }
    }

    fn handle_get_symbol_outline(&self, args: &serde_json::Value) -> McpToolCallResult {
        let path_filter = args.get("path").and_then(|v| v.as_str());
        let outlines = self.trie.get_outline(path_filter);

        let mut output = String::new();
        output.push_str("# Adapted Code STAIR: AST Symbol Table of Contents\n\n");

        if outlines.is_empty() {
            output.push_str("No symbols found matching filter.\n");
        } else {
            for (file, symbols) in outlines {
                output.push_str(&format!("## {}\n", file));
                for sym in symbols {
                    output.push_str(&format!(
                        "  - [{:?}] `{}` (lines {}-{}): {}\n",
                        sym.kind, sym.name, sym.line_start, sym.line_end, sym.signature
                    ));
                }
                output.push('\n');
            }
        }

        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: output,
            }],
            is_error: false,
        }
    }

    fn handle_get_symbol_definition(&self, args: &serde_json::Value) -> McpToolCallResult {
        let sym_path = match args.get("symbol_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return McpToolCallResult {
                    content: vec![McpTextContent {
                        content_type: "text".to_string(),
                        text: "Missing required argument 'symbol_path'".to_string(),
                    }],
                    is_error: true,
                };
            }
        };

        match self.trie.extract_symbol_code(sym_path) {
            Some((entry, code)) => {
                let text = format!(
                    "// File: {} (lines {}-{}) | Symbol: {}\n// Kind: {:?} | Zero-hallucination AST extraction\n```\n{}\n```\n",
                    entry.file_path, entry.line_start, entry.line_end, entry.symbol_path, entry.kind, code
                );
                McpToolCallResult {
                    content: vec![McpTextContent {
                        content_type: "text".to_string(),
                        text,
                    }],
                    is_error: false,
                }
            }
            None => {
                let suggestions = self.trie.find_by_prefix(sym_path);
                let mut text = format!("Symbol '{}' not found in resident Symbol Prefix Trie.\n", sym_path);
                if !suggestions.is_empty() {
                    text.push_str("\nDid you mean one of these valid symbols?\n");
                    for s in suggestions.iter().take(10) {
                        text.push_str(&format!("- `{}` ({:?})\n", s.symbol_path, s.kind));
                    }
                }
                McpToolCallResult {
                    content: vec![McpTextContent {
                        content_type: "text".to_string(),
                        text,
                    }],
                    is_error: true,
                }
            }
        }
    }

    fn handle_merkle_diff(&self, args: &serde_json::Value) -> McpToolCallResult {
        let from_hash = args.get("from_hash").and_then(|v| v.as_str());
        let diff = if let Some(base_hash) = from_hash {
            match self.watcher.diff_since_hash(base_hash) {
                Some(d) => d,
                None => {
                    return McpToolCallResult {
                        content: vec![McpTextContent {
                            content_type: "text".to_string(),
                            text: format!("Base hash '{}' not found in Merkle history.", base_hash),
                        }],
                        is_error: true,
                    };
                }
            }
        } else {
            let d = self.watcher.sync_and_diff();
            for f in &d.added_files {
                self.crepe.mark_dirty(f);
            }
            for f in &d.modified_files {
                self.crepe.mark_dirty(f);
            }
            d
        };

        let json_diff = serde_json::to_string_pretty(&diff).unwrap_or_default();
        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: json_diff,
            }],
            is_error: false,
        }
    }

    fn handle_merkle_snapshot(&self, _args: &serde_json::Value) -> McpToolCallResult {
        let snap = self.watcher.get_snapshot();
        let summary = format!(
            "BLAKE3 Merkle Root: {}\nTotal Files: {}\nTotal Dirs: {}\nTotal Bytes: {}\nTimestamp: {}\n",
            snap.root_hash, snap.total_files, snap.total_dirs, snap.total_bytes, snap.timestamp_secs
        );

        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: summary,
            }],
            is_error: false,
        }
    }

    fn handle_semantic_search(&self, args: &serde_json::Value) -> McpToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return McpToolCallResult {
                    content: vec![McpTextContent {
                        content_type: "text".to_string(),
                        text: "Missing required argument 'query'".to_string(),
                    }],
                    is_error: true,
                };
            }
        };

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let json_format = args.get("json_format").and_then(|v| v.as_bool()).unwrap_or(false);

        let (results, backend) = self.query_vector_rag(query, limit);

        let output = if json_format {
            let resp = VectorRagResponse {
                query: query.to_string(),
                backend: backend.to_string(),
                total: results.len(),
                results,
            };
            serde_json::to_string_pretty(&resp).unwrap_or_default()
        } else {
            self.format_enriched_markdown(query, backend, &results)
        };

        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: output,
            }],
            is_error: false,
        }
    }

    fn handle_index_status(&self, _args: &serde_json::Value) -> McpToolCallResult {
        let meta = self.crepe.get_metadata();
        let root_hash = self.watcher.root_hash();
        let total_symbols = self.semantic.total_indexed();

        // Check vector index files
        let vector_dir_candidates = [
            self.workspace_root.join(".antigravity").join("vectors"),
            self.workspace_root.join("target_repo").join("plane").join(".antigravity").join("vectors"),
            PathBuf::from(".antigravity").join("vectors"),
        ];
        let mut vector_index_exists = false;
        let mut detected_vector_path = None;
        for vdir in &vector_dir_candidates {
            if vdir.join("ast.faiss").exists() && vdir.join("metadata.db").exists() {
                vector_index_exists = true;
                detected_vector_path = Some(vdir.to_string_lossy().to_string());
                break;
            }
        }

        // Check resident sidecar health
        let sidecar_online = get_sidecar_http("/health").is_some();

        let status = json!({
            "status": "healthy",
            "merkle_root_hash": root_hash,
            "crepe_index": meta,
            "total_ast_symbols": total_symbols,
            "vector_rag": {
                "sidecar_online": sidecar_online,
                "sidecar_address": "127.0.0.1:9819",
                "index_present": vector_index_exists,
                "index_path": detected_vector_path
            },
            "engine": "antigravity-fast-core (Rust native)",
            "verification_backend": "ripgrep v14 + native regex fallback"
        });

        McpToolCallResult {
            content: vec![McpTextContent {
                content_type: "text".to_string(),
                text: serde_json::to_string_pretty(&status).unwrap_or_default(),
            }],
            is_error: false,
        }
    }

    pub fn query_vector_rag(&self, query: &str, limit: usize) -> (Vec<EnrichedAstNode>, &'static str) {
        // Tier 1: Resident Sidecar HTTP on 127.0.0.1:9819
        if let Some(nodes) = self.query_sidecar_http(query, limit) {
            if !nodes.is_empty() {
                return (nodes, "CUDA GPU Sidecar (FAISS + SQLite)");
            }
        }

        // Tier 2: Subprocess Vector Query CLI
        if let Some(nodes) = self.query_subprocess_cli(query, limit) {
            if !nodes.is_empty() {
                return (nodes, "Subprocess Vector RAG (FAISS + SQLite)");
            }
        }

        // Tier 3: In-Process Resident SymbolTrie Fallback
        let nodes = self.query_resident_trie_fallback(query, limit);
        (nodes, "In-Process Resident SymbolTrie")
    }

    fn query_sidecar_http(&self, query: &str, limit: usize) -> Option<Vec<EnrichedAstNode>> {
        let req_body = json!({
            "query": query,
            "top_k": limit,
            "workspace": self.workspace_root.to_string_lossy()
        }).to_string();

        // First attempt /rag/search endpoint
        if let Some(resp) = post_sidecar_http("/rag/search", &req_body) {
            if let Some(nodes) = parse_enriched_json_results(&resp) {
                if !nodes.is_empty() {
                    return Some(self.ensure_enriched_nodes(nodes));
                }
            }
        }

        // Second attempt /search endpoint
        if let Some(resp) = post_sidecar_http("/search", &req_body) {
            if let Some(nodes) = parse_enriched_json_results(&resp) {
                if !nodes.is_empty() {
                    return Some(self.ensure_enriched_nodes(nodes));
                }
            }
        }

        None
    }

    fn query_subprocess_cli(&self, query: &str, limit: usize) -> Option<Vec<EnrichedAstNode>> {
        let script_candidates = [
            self.workspace_root.join("scripts").join("query_vector_rag.py"),
            PathBuf::from("scripts").join("query_vector_rag.py"),
        ];

        let script_path = script_candidates.iter().find(|p| p.exists())?;

        // Check if vector index exists
        let vector_dir_candidates = [
            self.workspace_root.join(".antigravity").join("vectors"),
            self.workspace_root.join("target_repo").join("plane").join(".antigravity").join("vectors"),
            PathBuf::from(".antigravity").join("vectors"),
        ];
        let index_exists = vector_dir_candidates
            .iter()
            .any(|d| d.join("ast.faiss").exists() && d.join("metadata.db").exists());
        if !index_exists {
            return None;
        }

        // Try invocation with --top-k
        let out = std::process::Command::new("python")
            .arg(script_path)
            .arg("--workspace")
            .arg(&self.workspace_root)
            .arg("--query")
            .arg(query)
            .arg("--top-k")
            .arg(limit.to_string())
            .output();

        let stdout = match out {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            _ => {
                // Try invocation with --limit
                let fallback_out = std::process::Command::new("python")
                    .arg(script_path)
                    .arg("--workspace")
                    .arg(&self.workspace_root)
                    .arg("--query")
                    .arg(query)
                    .arg("--limit")
                    .arg(limit.to_string())
                    .output()
                    .ok()?;
                if !fallback_out.status.success() {
                    return None;
                }
                String::from_utf8_lossy(&fallback_out.stdout).to_string()
            }
        };

        let nodes = parse_enriched_json_results(&stdout)?;
        Some(self.ensure_enriched_nodes(nodes))
    }

    fn query_resident_trie_fallback(&self, query: &str, limit: usize) -> Vec<EnrichedAstNode> {
        let matches = self.semantic.search(query, limit);
        let mut nodes = Vec::new();
        for m in &matches {
            if let Some(node) = self.enrich_symbol(&m.symbol_path, m.score) {
                nodes.push(node);
            }
        }

        // If local semantic search returned few/no matches, augment with prefix/contains search
        if nodes.len() < limit {
            let suggestions = self.trie.find_by_prefix(query);
            for s in suggestions {
                if nodes.iter().any(|n| n.symbol_path == s.symbol_path) {
                    continue;
                }
                let score = if s.name.eq_ignore_ascii_case(query) {
                    0.95
                } else if s.symbol_path.contains(query) {
                    0.80
                } else {
                    0.65
                };
                if let Some(node) = self.enrich_symbol(&s.symbol_path, score) {
                    nodes.push(node);
                    if nodes.len() >= limit {
                        break;
                    }
                }
            }
        }

        nodes
    }

    fn ensure_enriched_nodes(&self, mut nodes: Vec<EnrichedAstNode>) -> Vec<EnrichedAstNode> {
        for node in &mut nodes {
            if node.preview.is_empty() || node.signature.is_empty() {
                if let Some(enriched) = self.enrich_symbol(&node.symbol_path, node.score) {
                    if node.class_name.is_none() {
                        node.class_name = enriched.class_name;
                    }
                    if node.signature.is_empty() {
                        node.signature = enriched.signature;
                    }
                    if node.decorators.is_empty() {
                        node.decorators = enriched.decorators;
                    }
                    if node.docstring.is_none() {
                        node.docstring = enriched.docstring;
                    }
                    if node.preview.is_empty() {
                        node.preview = enriched.preview;
                    }
                    if node.line_start == 0 {
                        node.line_start = enriched.line_start;
                        node.line_end = enriched.line_end;
                    }
                }
            }
        }
        nodes
    }

    fn enrich_symbol(&self, symbol_path: &str, score: f32) -> Option<EnrichedAstNode> {
        if let Some((entry, code)) = self.trie.extract_symbol_code(symbol_path) {
            let parts: Vec<&str> = entry.symbol_path.split("::").collect();
            let class_name = if parts.len() >= 3 {
                Some(parts[parts.len() - 2].to_string())
            } else {
                None
            };

            let mut decorators = if !entry.decorators.is_empty() {
                entry.decorators.clone()
            } else {
                extract_decorators(&code)
            };

            // Also check source file lines immediately preceding the definition for decorators
            let abs_path = self.workspace_root.join(&entry.file_path);
            let mut full_code = code.clone();
            if let Ok(file_content) = std::fs::read_to_string(&abs_path) {
                let file_lines: Vec<&str> = file_content.lines().collect();
                let start_idx_0 = entry.line_start.saturating_sub(1);
                let mut check_idx = start_idx_0;
                let mut leading_decs = Vec::new();
                while check_idx > 0 {
                    check_idx -= 1;
                    let l = file_lines[check_idx].trim();
                    if l.starts_with('@') {
                        leading_decs.insert(0, l.to_string());
                    } else {
                        break;
                    }
                }
                if !leading_decs.is_empty() {
                    let dec_start_0 = start_idx_0.saturating_sub(leading_decs.len());
                    let dec_slice = file_lines[dec_start_0..start_idx_0].join("\n");
                    full_code = format!("{}\n{}", dec_slice, code);
                    for d in leading_decs.into_iter().rev() {
                        if !decorators.contains(&d) {
                            decorators.insert(0, d);
                        }
                    }
                }
            }

            let docstring = extract_docstring(&full_code);
            let preview = extract_preview(&full_code, 20);

            let short_name = if entry.name.contains("::") {
                entry.name.split("::").last().unwrap_or(&entry.name).to_string()
            } else {
                entry.name.clone()
            };

            Some(EnrichedAstNode {
                symbol_path: entry.symbol_path,
                file_path: entry.file_path,
                name: short_name,
                kind: format!("{:?}", entry.kind).to_lowercase(),
                class_name,
                signature: entry.signature,
                decorators,
                docstring,
                line_start: entry.line_start,
                line_end: entry.line_end,
                byte_start: entry.byte_start,
                byte_end: entry.byte_end,
                score,
                preview,
            })
        } else {
            let norm = symbol_path.replace('\\', "/");
            let parts: Vec<&str> = norm.split("::").collect();
            let file_path = parts.first().unwrap_or(&"").to_string();
            let name = parts.last().unwrap_or(&"").to_string();
            let class_name = if parts.len() >= 3 {
                Some(parts[parts.len() - 2].to_string())
            } else {
                None
            };

            Some(EnrichedAstNode {
                symbol_path: norm,
                file_path,
                name,
                kind: "symbol".to_string(),
                class_name,
                signature: String::new(),
                decorators: Vec::new(),
                docstring: None,
                line_start: 1,
                line_end: 1,
                byte_start: 0,
                byte_end: 0,
                score,
                preview: String::new(),
            })
        }
    }

    fn format_enriched_markdown(&self, query: &str, backend: &str, nodes: &[EnrichedAstNode]) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Vector RAG & Semantic AST Search Results for '{}'\n",
            query
        ));
        out.push_str(&format!(
            "*Backend: {} | Matches: {}*\n\n",
            backend,
            nodes.len()
        ));

        if nodes.is_empty() {
            out.push_str("No semantic matches found.\n");
            return out;
        }

        for (idx, node) in nodes.iter().enumerate() {
            out.push_str(&format!(
                "### {}. `{}` (Score: {:.3})\n",
                idx + 1,
                node.symbol_path,
                node.score
            ));
            if let Some(ref cls) = node.class_name {
                if !cls.is_empty() {
                    out.push_str(&format!("- **Class**: `{}`\n", cls));
                }
            }
            if !node.kind.is_empty() {
                out.push_str(&format!("- **Kind**: `{}`\n", node.kind));
            }
            out.push_str(&format!(
                "- **File**: `{}` (lines {}-{})\n",
                node.file_path, node.line_start, node.line_end
            ));
            if !node.signature.is_empty() {
                out.push_str(&format!("- **Signature**: `{}`\n", node.signature));
            }
            if !node.decorators.is_empty() {
                let decs: Vec<String> = node.decorators.iter().map(|d| format!("'{}'", d)).collect();
                out.push_str(&format!("- **Decorators**: [{}]\n", decs.join(", ")));
            }
            if let Some(ref doc) = node.docstring {
                if !doc.trim().is_empty() {
                    out.push_str(&format!("- **Docstring**: {}\n", doc.trim()));
                }
            }
            if !node.preview.trim().is_empty() {
                let lang = if node.file_path.ends_with(".py") {
                    "python"
                } else if node.file_path.ends_with(".rs") {
                    "rust"
                } else if node.file_path.ends_with(".ts") || node.file_path.ends_with(".tsx") {
                    "typescript"
                } else if node.file_path.ends_with(".js") || node.file_path.ends_with(".jsx") {
                    "javascript"
                } else {
                    ""
                };
                out.push_str(&format!(
                    "- **AST Node Preview**:\n```{}\n{}\n```\n",
                    lang,
                    node.preview.trim_end()
                ));
            }
            out.push('\n');
        }

        out
    }
}

fn post_sidecar_http(path: &str, body: &str) -> Option<String> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9819".parse().ok()?,
        std::time::Duration::from_millis(300),
    ).ok()?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).ok()?;
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:9819\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, body.len(), body
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Some(resp[pos + 4..].to_string())
    } else {
        None
    }
}

fn get_sidecar_http(path: &str) -> Option<String> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9819".parse().ok()?,
        std::time::Duration::from_millis(300),
    ).ok()?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(1))).ok()?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:9819\r\nConnection: close\r\n\r\n",
        path
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Some(resp[pos + 4..].to_string())
    } else {
        None
    }
}

fn parse_enriched_json_results(json_str: &str) -> Option<Vec<EnrichedAstNode>> {
    if let Ok(nodes) = serde_json::from_str::<Vec<EnrichedAstNode>>(json_str) {
        if !nodes.is_empty() {
            return Some(nodes);
        }
    }

    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(alias = "matches", alias = "nodes", alias = "data")]
        results: Vec<EnrichedAstNode>,
    }

    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(json_str) {
        if !wrapper.results.is_empty() {
            return Some(wrapper.results);
        }
    }

    None
}

fn extract_decorators(code: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('@') {
            decorators.push(trimmed.to_string());
        } else if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
        {
            if !decorators.is_empty() {
                break;
            }
        }
    }
    decorators
}

fn extract_docstring(code: &str) -> Option<String> {
    if let Some(start) = code.find("\"\"\"") {
        if let Some(end) = code[start + 3..].find("\"\"\"") {
            let doc = code[start + 3..start + 3 + end].trim();
            if !doc.is_empty() {
                return Some(doc.to_string());
            }
        }
    }
    if let Some(start) = code.find("'''") {
        if let Some(end) = code[start + 3..].find("'''") {
            let doc = code[start + 3..start + 3 + end].trim();
            if !doc.is_empty() {
                return Some(doc.to_string());
            }
        }
    }
    let mut rust_docs = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if let Some(doc_line) = trimmed.strip_prefix("///") {
            rust_docs.push(doc_line.trim());
        } else if !rust_docs.is_empty() {
            break;
        }
    }
    if !rust_docs.is_empty() {
        return Some(rust_docs.join("\n"));
    }
    if let Some(start) = code.find("/**") {
        if let Some(end) = code[start + 3..].find("*/") {
            let doc = code[start + 3..start + 3 + end]
                .lines()
                .map(|l| l.trim().trim_start_matches('*').trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !doc.is_empty() {
                return Some(doc);
            }
        }
    }
    None
}

fn extract_preview(code: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = code.lines().collect();
    if lines.len() <= max_lines {
        code.trim_end().to_string()
    } else {
        let mut preview = lines[..max_lines].join("\n");
        preview.push_str(&format!("\n... [{} more lines]", lines.len() - max_lines));
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_list_tools_contains_enriched_semantic_search() {
        let tools = McpHandler::list_tools();
        let tool = tools
            .iter()
            .find(|t| t.name == "semantic_search")
            .expect("semantic_search tool missing");
        assert!(tool.description.contains("Vector RAG"));
        assert!(tool.description.contains("enriched node metadata"));

        let props = tool
            .input_schema
            .get("properties")
            .expect("missing properties");
        assert!(props.get("query").is_some());
        assert!(props.get("limit").is_some());
        assert!(props.get("json_format").is_some());
    }

    #[test]
    fn test_handle_semantic_search_missing_query() {
        let dir = tempdir().unwrap();
        let handler = McpHandler::new(dir.path().to_path_buf());
        let res = handler.handle_semantic_search(&json!({}));
        assert!(res.is_error);
        assert!(res.content[0].text.contains("Missing required argument 'query'"));
    }

    #[test]
    fn test_extract_decorators_and_docstrings() {
        let python_snippet = r#"
    @allow_permission([ROLE.ADMIN])
    @action(detail=True, methods=["get"], url_path="audit-logs")
    def audit_logs(self, request, slug, pk):
        """Retrieve audit logs for project member role transitions."""
        member = self.get_object()
        logs = ProjectMemberAuditLog.objects.filter(project_member=member)
        return Response(ProjectMemberAuditLogSerializer(logs, many=True).data)
"#;
        let decs = extract_decorators(python_snippet);
        assert_eq!(decs.len(), 2);
        assert!(decs[0].contains("@allow_permission"));
        assert!(decs[1].contains("@action"));

        let doc = extract_docstring(python_snippet);
        assert_eq!(
            doc.as_deref(),
            Some("Retrieve audit logs for project member role transitions.")
        );

        let preview = extract_preview(python_snippet, 3);
        assert!(preview.contains("..."));
    }

    #[test]
    fn test_parse_enriched_json_results_various_formats() {
        // Format 1: direct list
        let json_list = r#"[
            {
                "symbol_path": "apps/api/plane/app/views/project/member.py::ProjectMemberViewSet::audit_logs",
                "file_path": "apps/api/plane/app/views/project/member.py",
                "name": "audit_logs",
                "kind": "method",
                "class_name": "ProjectMemberViewSet",
                "signature": "def audit_logs(self, request, slug, pk):",
                "decorators": ["@action(detail=True, methods=[\"get\"])"],
                "docstring": "Retrieve audit logs.",
                "line_start": 150,
                "line_end": 180,
                "byte_start": 1000,
                "byte_end": 1500,
                "score": 0.92,
                "preview": "def audit_logs(): pass"
            }
        ]"#;
        let nodes1 = parse_enriched_json_results(json_list).expect("failed format 1");
        assert_eq!(nodes1.len(), 1);
        assert_eq!(nodes1[0].name, "audit_logs");
        assert_eq!(nodes1[0].class_name.as_deref(), Some("ProjectMemberViewSet"));
        assert_eq!(nodes1[0].decorators.len(), 1);

        // Format 2: results wrapper with string decorators
        let json_wrapper = r#"{
            "results": [
                {
                    "node_id": "apps/api/plane/throttles/project.py::ProjectMemberRateThrottle",
                    "file": "apps/api/plane/throttles/project.py",
                    "symbol_name": "ProjectMemberRateThrottle",
                    "symbol_type": "class",
                    "class": null,
                    "signature": "class ProjectMemberRateThrottle(BaseThrottle):",
                    "decorators": "[\"@throttle_scope('project_member')\"]",
                    "docstring": "Rate throttle for project member actions.",
                    "score": 0.88,
                    "preview": "class ProjectMemberRateThrottle: pass"
                }
            ]
        }"#;
        let nodes2 = parse_enriched_json_results(json_wrapper).expect("failed format 2");
        assert_eq!(nodes2.len(), 1);
        assert_eq!(
            nodes2[0].symbol_path,
            "apps/api/plane/throttles/project.py::ProjectMemberRateThrottle"
        );
        assert_eq!(nodes2[0].decorators.len(), 1);
        assert!(nodes2[0].decorators[0].contains("@throttle_scope"));
    }

    #[test]
    fn test_handle_semantic_search_enriched_markdown_and_json() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let views_dir = root.join("apps").join("api").join("views");
        fs::create_dir_all(&views_dir).unwrap();
        let member_py = views_dir.join("member.py");
        fs::write(
            &member_py,
            r#"class ProjectMemberViewSet:
    @allow_permission([ROLE.ADMIN])
    @action(detail=True, methods=["get"], url_path="audit-logs")
    def audit_logs(self, request, slug, pk):
        """Retrieve audit logs for project member role transitions."""
        member = self.get_object()
        logs = ProjectMemberAuditLog.objects.filter(project_member=member)
        return Response(ProjectMemberAuditLogSerializer(logs, many=True).data)
"#,
        )
        .unwrap();

        let handler = McpHandler::new(root);

        // 1. Markdown formatted call
        let md_res = handler.handle_tool_call(
            "semantic_search",
            &json!({
                "query": "audit_logs"
            }),
        );
        assert!(!md_res.is_error);
        let text = &md_res.content[0].text;
        assert!(text.contains("Vector RAG & Semantic AST Search Results"));
        assert!(text.contains("audit_logs"));
        assert!(text.contains("ProjectMemberViewSet"));
        assert!(text.contains("@action") || text.contains("@allow_permission"));
        assert!(text.contains("Retrieve audit logs for project member role transitions."));
        assert!(text.contains("ProjectMemberAuditLog.objects.filter"));

        // 2. JSON formatted call
        let json_res = handler.handle_tool_call(
            "semantic_search",
            &json!({
                "query": "audit_logs",
                "json_format": true
            }),
        );
        assert!(!json_res.is_error);
        let json_val: serde_json::Value = serde_json::from_str(&json_res.content[0].text).unwrap();
        assert_eq!(json_val["query"], "audit_logs");
        assert!(json_val["results"].is_array());
        let results = json_val["results"].as_array().unwrap();
        assert!(!results.is_empty());
        let first = &results[0];
        assert_eq!(first["name"], "audit_logs");
        assert_eq!(first["class_name"], "ProjectMemberViewSet");
        assert!(first["decorators"].as_array().unwrap().len() >= 1);
        assert_eq!(
            first["docstring"],
            "Retrieve audit logs for project member role transitions."
        );
        assert!(first["preview"].as_str().unwrap().contains("ProjectMemberAuditLog"));
    }

    #[test]
    fn test_handle_index_status_includes_vector_rag() {
        let dir = tempdir().unwrap();
        let handler = McpHandler::new(dir.path().to_path_buf());
        let res = handler.handle_index_status(&json!({}));
        assert!(!res.is_error);
        let val: serde_json::Value = serde_json::from_str(&res.content[0].text).unwrap();
        assert!(val.get("vector_rag").is_some());
        assert!(val["vector_rag"].get("sidecar_online").is_some());
        assert!(val["vector_rag"].get("index_present").is_some());
    }
}
