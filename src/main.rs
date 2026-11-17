mod bench;
mod crepe;
mod embeddings;
mod installer;
mod mcp;
mod merkle;
mod stair;

use bench::{BenchmarkRunner, Scalability50kRunner};
use clap::Parser;
use crepe::{CrepeIndex, Verifier};
use embeddings::LocalSemanticIndex;
use merkle::MerkleTree;
use stair::SymbolTrie;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "antigravity-fast-core",
    version = "0.1.0",
    about = "High-speed native code intelligence layer: BLAKE3 Merkle tree, Crepe sparse n-gram index, Code STAIR symbol trie, and stdio MCP server"
)]
struct Cli {
    /// Workspace root directory (default: current directory)
    #[arg(short, long, default_value = ".")]
    workspace: PathBuf,

    /// Setup and register Antigravity Fast-Core globally for production
    #[arg(long)]
    setup: bool,

    /// Display active Fable-Mode execution guardrails and agent discipline
    #[arg(long)]
    guardrails: bool,

    /// List all indexed workspaces in central cache
    #[arg(long)]
    cache_list: bool,

    /// Clean central cache for current workspace
    #[arg(long)]
    cache_clean: bool,

    /// Clean all central caches across all workspaces
    #[arg(long)]
    cache_clean_all: bool,

    /// Run empirical benchmarks comparing against PowerShell/Select-String baseline
    #[arg(short, long)]
    benchmark: bool,

    /// Run massive 50,000-file empirical scalability benchmark (generates or benchmarks 50k repo)
    #[arg(long)]
    benchmark_50k: bool,

    /// Force rebuilding Crepe sparse n-gram index
    #[arg(long)]
    index: bool,

    /// Search workspace using instant crepe grep
    #[arg(short, long)]
    grep: Option<String>,

    /// Print AST Symbol Table of Contents outline
    #[arg(short, long)]
    outline: Option<Option<String>>,

    /// Extract exact symbol code definition from Symbol Prefix Trie
    #[arg(short, long)]
    def: Option<String>,

    /// Natural language or subword query to locate target symbol (1-shot surgical context lens)
    #[arg(short, long)]
    query: Option<String>,

    /// Path filter for query or outline
    #[arg(short = 'p', long)]
    path: Option<String>,

    /// Prepend file import header to symbol slice
    #[arg(long)]
    imports: bool,

    /// Show current BLAKE3 Merkle tree root and snapshot
    #[arg(short, long)]
    merkle: bool,

    /// Run microsecond dirty-file Merkle diff and fast state check
    #[arg(long)]
    verify: bool,

    /// Pure Rust semantic search across symbols using subword BM25/TF-IDF
    #[arg(short, long)]
    semantic: Option<String>,

    /// Autonomous pure Rust adaptive cascade (Grep -> Semantic -> STAIR Slice)
    #[arg(short, long)]
    cascade: Option<String>,

    /// Run stdio MCP server (default if no other command specified)
    #[arg(long)]
    mcp: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let workspace = mcp::pool::normalize_path(&cli.workspace);

    if cli.setup {
        installer::run_setup()?;
        return Ok(());
    }

    if cli.guardrails {
        println!("{}", include_str!("../rules/execution-guardrails.md"));
        return Ok(());
    }

    if cli.cache_list {
        println!("Central Cache Directory: {:?}", crepe::cache_mgr::get_base_cache_dir());
        let list = crepe::cache_mgr::list_caches();
        if list.is_empty() {
            println!("No cached workspaces found.");
        } else {
            println!("\nFound {} cached workspace(s):", list.len());
            for (dir, meta) in list {
                println!("- [{}] {}", meta.workspace_name, meta.workspace_path);
                println!("    Cache Dir: {:?}", dir);
                println!("    Indexed Files: {} | Size: {:.2} MB | Last Accessed: {}",
                    meta.total_files,
                    meta.total_bytes as f64 / 1_048_576.0,
                    meta.last_accessed);
            }
        }
        return Ok(());
    }

    if cli.cache_clean {
        let removed = crepe::cache_mgr::clean_cache(&workspace)?;
        if removed {
            println!("Cleaned cache for workspace: {:?}", workspace);
        } else {
            println!("No cache found for workspace: {:?}", workspace);
        }
        return Ok(());
    }

    if cli.cache_clean_all {
        let count = crepe::cache_mgr::clean_all_caches()?;
        println!("Cleaned {} workspace cache(s).", count);
        return Ok(());
    }

    if cli.benchmark {
        let runner = BenchmarkRunner::new(workspace.clone());
        let report = runner.run_suite();
        let report_path = workspace.join("BENCHMARK_RESULTS.md");
        fs::write(&report_path, &report.summary_markdown)?;
        println!("Benchmark report saved to: {:?}", report_path);
        return Ok(());
    }

    if cli.benchmark_50k {
        let bench_dir = workspace.join("benchmark_50k_repo");
        let runner = Scalability50kRunner::new(bench_dir);
        let report = runner.run_suite();
        let report_path = workspace.join("BENCHMARK_50K_REPORT.md");
        fs::write(&report_path, &report.markdown_summary)?;
        println!("50,000-File Scalability Benchmark report saved to: {:?}", report_path);
        return Ok(());
    }

    if cli.index {
        println!("Building Crepe sparse n-gram index for {:?}", workspace);
        let crepe = CrepeIndex::new(workspace.clone());
        let meta = crepe.build()?;
        println!("Indexed {} files, {} unique n-grams in {} ms", meta.total_files, meta.total_ngrams, meta.build_duration_ms);
        return Ok(());
    }

    if let Some(query) = cli.grep {
        let crepe = CrepeIndex::new(workspace.clone());
        if crepe.total_files() == 0 {
            println!("Crepe index not found. Building index first...");
            crepe.build()?;
        }
        let verifier = Verifier::new(workspace.clone());
        let t0 = std::time::Instant::now();
        let candidates = crepe.find_candidates(&query);
        let matches = verifier.verify_in_process(&query, &candidates, false)?;
        let dur = t0.elapsed();
        println!("Instant Grep for '{}' (Found {} matches across {}/{} candidate files in {:.2} ms):",
            query, matches.len(), candidates.len(), crepe.total_files(), dur.as_secs_f64() * 1000.0);
        for m in matches.iter().take(20) {
            println!("{}:{}:{}: {}", m.file, m.line_number, m.column, m.line_content.trim());
        }
        return Ok(());
    }

    if let Some(path_opt) = cli.outline {
        let trie = SymbolTrie::new(workspace.clone());
        let count = match path_opt.as_deref() {
            Some(target) if !target.trim().is_empty() => trie.build_scoped(target),
            _ => trie.build(),
        };
        println!("AST Symbol Outline ({} symbols):", count);
        let filter = path_opt.as_deref();
        for (file, symbols) in trie.get_outline(filter) {
            println!("File: {}", file);
            for s in symbols {
                println!("  [{:?}] {} (L{}-L{}): {}", s.kind, s.name, s.line_start, s.line_end, s.signature);
            }
        }
        return Ok(());
    }

    if let Some(sym_path) = cli.def {
        let trie = SymbolTrie::new(workspace.clone());
        if let Some((entry, code)) = trie.extract_symbol_code(&sym_path) {
            println!("// Symbol: {} [{:?}] in {}:{}-{}", entry.symbol_path, entry.kind, entry.file_path, entry.line_start, entry.line_end);
            println!("{}", code);
            return Ok(());
        }

        trie.build();
        match trie.extract_symbol_code(&sym_path) {
            Some((entry, code)) => {
                println!("// Symbol: {} [{:?}] in {}:{}-{}", entry.symbol_path, entry.kind, entry.file_path, entry.line_start, entry.line_end);
                println!("{}", code);
            }
            None => {
                eprintln!("Symbol '{}' not found.", sym_path);
            }
        }
        return Ok(());
    }

    if let Some(query_str) = cli.query {
        let trie = SymbolTrie::new(workspace.clone());
        let _ = trie.build();
        let matches = trie.find_symbols_by_fuzzy_query(&query_str, cli.path.as_deref(), 5);
        if matches.is_empty() {
            println!("No symbols found matching query '{}'.", query_str);
            return Ok(());
        }
        println!("Surgical AST Context Lens: Found {} matches for '{}'", matches.len(), query_str);
        let top_match = &matches[0].0;
        if let Some(res) = trie.extract_symbol_with_context(&top_match.symbol_path, cli.imports, 0) {
            println!(
                "// File: {} (lines {}-{} / total {}) | Symbol: {}",
                res.entry.file_path, res.line_start_with_padding, res.line_end_with_padding, res.total_file_lines, res.entry.symbol_path
            );
            if let Some(ref imps) = res.imports_header {
                println!("// --- Imports Header ---\n{}\n// --- Symbol Code ---", imps);
            }
            println!("{}", res.code);
        }
        if matches.len() > 1 {
            println!("\n// Additional ranked matches:");
            for (m, score) in matches.iter().skip(1) {
                println!(
                    "// - `{}` ({:?} L{}-L{}, score: {:.1})",
                    m.symbol_path, m.kind, m.line_start, m.line_end, score
                );
            }
        }
        return Ok(());
    }

    if cli.merkle {
        let tree = MerkleTree::new(workspace.clone());
        let snap = tree.get_snapshot();
        println!("BLAKE3 Merkle Tree Root: {}", snap.root_hash);
        println!("Total Files: {} | Total Dirs: {} | Total Bytes: {}", snap.total_files, snap.total_dirs, snap.total_bytes);
        let diff = tree.refresh();
        println!("Dirty Diff Duration: {} µs (Changes: {})", diff.duration_micros, diff.total_changes);
        return Ok(());
    }

    if cli.verify {
        let tree = MerkleTree::new(workspace.clone());
        let snap = tree.get_snapshot();
        let diff = tree.refresh();
        println!("BLAKE3 Merkle State: Root={}", snap.root_hash);
        println!("Modified files ({}):", diff.total_changes);
        for f in &diff.added_files {
            println!("  + {}", f);
        }
        for f in &diff.modified_files {
            println!("  * {}", f);
        }
        for f in &diff.deleted_files {
            println!("  - {}", f);
        }
        println!("Diff computed in {} µs", diff.duration_micros);
        return Ok(());
    }

    if let Some(query) = cli.semantic {
        let trie = SymbolTrie::new(workspace.clone());
        let _ = trie.build();
        let index = LocalSemanticIndex::new();
        let mut items = Vec::new();
        for (file, symbols) in trie.get_outline(None) {
            for sym in symbols {
                let sym_path = format!("{}::{}", file, sym.name);
                items.push((sym_path, sym.signature, file.clone()));
            }
        }
        index.index_symbols_batch(items);
        let t0 = std::time::Instant::now();
        let results = index.search(&query, 10);
        let elapsed = t0.elapsed();
        println!("Pure Rust Semantic Search for '{}' (Found {} symbols in {:.2} ms):", query, results.len(), elapsed.as_secs_f64() * 1000.0);
        for r in results {
            println!("  [{:.3}] {} -> {}", r.score, r.symbol_path, r.signature);
        }
        return Ok(());
    }

    if let Some(query) = cli.cascade {
        let t0 = std::time::Instant::now();
        let crepe = CrepeIndex::new(workspace.clone());
        if crepe.total_files() == 0 {
            let _ = crepe.build();
        }
        let verifier = Verifier::new(workspace.clone());
        let candidates = crepe.find_candidates(&query);
        let matches = verifier.verify_in_process(&query, &candidates, false).unwrap_or_default();
        let trie = SymbolTrie::new(workspace.clone());

        if !matches.is_empty() {
            let elapsed = t0.elapsed();
            println!("[Tier 1: Instant Crepe Fast-Path] Found {} matches in {:.2} ms:", matches.len(), elapsed.as_secs_f64() * 1000.0);
            for m in matches.iter().take(5) {
                println!("{}:{}: {}", m.file, m.line_number, m.line_content.trim());
            }
            return Ok(());
        }

        let _ = trie.build();
        let index = LocalSemanticIndex::new();
        let mut items = Vec::new();
        for (file, symbols) in trie.get_outline(None) {
            for sym in symbols {
                let sym_path = format!("{}::{}", file, sym.name);
                items.push((sym_path, sym.signature, file.clone()));
            }
        }
        index.index_symbols_batch(items);
        let results = index.search(&query, 5);
        let elapsed = t0.elapsed();
        println!("[Tier 3: Pure Rust Semantic Fallback] (Grep 0 hits, resolved in {:.2} ms):", elapsed.as_secs_f64() * 1000.0);
        for r in results {
            println!("  [{:.3}] {} -> {}", r.score, r.symbol_path, r.signature);
            if let Some((_, code)) = trie.extract_symbol_code(&r.symbol_path) {
                println!("// [Tier 2: STAIR AST Slice for {}]:", r.symbol_path);
                println!("{}", code.lines().take(20).collect::<Vec<_>>().join("\n"));
                break;
            }
        }
        return Ok(());
    }

    // Default: Run MCP stdio server
    mcp::run_stdio_server(workspace)?;
    Ok(())
}
