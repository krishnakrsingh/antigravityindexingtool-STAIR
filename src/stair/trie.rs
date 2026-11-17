use super::ast::{AstOutlineParser, CodeSymbol, SymbolKind};
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub symbol_path: String,
    pub file_path: String,
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    #[serde(default)]
    pub decorators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContextResult {
    pub entry: SymbolEntry,
    pub code: String,
    pub imports_header: Option<String>,
    pub line_start_with_padding: usize,
    pub line_end_with_padding: usize,
    pub total_file_lines: usize,
}

#[derive(Default)]
struct TrieNode {
    entry: Option<SymbolEntry>,
    children: HashMap<String, TrieNode>,
}

pub struct SymbolTrie {
    workspace_root: PathBuf,
    root: RwLock<TrieNode>,
    symbols_by_path: RwLock<HashMap<String, SymbolEntry>>,
    file_symbols: RwLock<HashMap<String, Vec<CodeSymbol>>>,
}

impl SymbolTrie {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            root: RwLock::new(TrieNode::default()),
            symbols_by_path: RwLock::new(HashMap::new()),
            file_symbols: RwLock::new(HashMap::new()),
        }
    }

    /// Indexes all source files into the Symbol Trie
    pub fn build(&self) -> usize {
        let mut walker = WalkBuilder::new(&self.workspace_root);
        walker
            .hidden(true)
            .git_ignore(true)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".git" && name != "target" && name != "node_modules" && name != ".antigravity"
            });

        let file_paths: Vec<PathBuf> = walker
            .build()
            .filter_map(|res| res.ok())
            .map(|entry| entry.into_path())
            .filter(|p| p.is_file())
            .collect();

        let parsed: Vec<(String, Vec<CodeSymbol>)> = file_paths
            .par_iter()
            .filter_map(|p| {
                let rel = p.strip_prefix(&self.workspace_root).ok()?;
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let content = fs::read_to_string(p).ok()?;
                let symbols = AstOutlineParser::parse_file(p, &content);
                if symbols.is_empty() {
                    None
                } else {
                    Some((rel_str, symbols))
                }
            })
            .collect();

        let mut count = 0;
        let mut trie_root = TrieNode::default();
        let mut symbols_by_path = HashMap::with_capacity(parsed.len() * 3);
        let mut file_symbols = HashMap::with_capacity(parsed.len());

        for (rel_str, symbols) in parsed {
            for sym in &symbols {
                let sym_path = format!("{}::{}", rel_str, sym.name);
                let entry = SymbolEntry {
                    symbol_path: sym_path.clone(),
                    file_path: rel_str.clone(),
                    name: sym.name.clone(),
                    kind: sym.kind,
                    signature: sym.signature.clone(),
                    line_start: sym.line_start,
                    line_end: sym.line_end,
                    byte_start: sym.byte_start,
                    byte_end: sym.byte_end,
                    decorators: sym.decorators.clone(),
                };

                Self::insert_in_node(&mut trie_root, &sym_path, entry.clone());
                symbols_by_path.insert(sym_path.clone(), entry.clone());

                // If name is Type::method (e.g. CrepeIndex::find_candidates), also alias file::method
                if sym.name.contains("::") {
                    let method_part = sym.name.split("::").last().unwrap_or(&sym.name);
                    let short_path = format!("{}::{}", rel_str, method_part);
                    symbols_by_path.entry(short_path.clone()).or_insert_with(|| {
                        let mut short_entry = entry.clone();
                        short_entry.symbol_path = short_path;
                        short_entry
                    });
                }

                count += 1;
            }
            file_symbols.insert(rel_str, symbols);
        }

        *self.root.write().unwrap() = trie_root;
        *self.symbols_by_path.write().unwrap() = symbols_by_path;
        *self.file_symbols.write().unwrap() = file_symbols;

        count
    }

    fn insert_in_node(node: &mut TrieNode, path: &str, entry: SymbolEntry) {
        let parts: Vec<&str> = path.split("::").collect();
        let mut curr = node;
        for part in parts {
            curr = curr.children.entry(part.to_string()).or_default();
        }
        curr.entry = Some(entry);
    }

    /// Fast scoped indexing for a specific file or directory
    pub fn build_scoped(&self, target_path: &str) -> usize {
        let norm_target = target_path.replace('\\', "/");
        let target_abs = self.workspace_root.join(&norm_target);
        
        let file_paths: Vec<PathBuf> = if target_abs.is_file() {
            vec![target_abs]
        } else if target_abs.is_dir() {
            let mut walker = WalkBuilder::new(&target_abs);
            walker
                .hidden(true)
                .git_ignore(true)
                .filter_entry(|entry| {
                    let name = entry.file_name().to_string_lossy();
                    name != ".git" && name != "target" && name != "node_modules" && name != ".antigravity"
                });
            walker
                .build()
                .filter_map(|res| res.ok())
                .map(|entry| entry.into_path())
                .filter(|p| p.is_file())
                .collect()
        } else {
            return 0;
        };

        let parsed: Vec<(String, Vec<CodeSymbol>)> = file_paths
            .par_iter()
            .filter_map(|p| {
                let rel = p.strip_prefix(&self.workspace_root).ok()?;
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let content = fs::read_to_string(p).ok()?;
                let symbols = AstOutlineParser::parse_file(p, &content);
                if symbols.is_empty() {
                    None
                } else {
                    Some((rel_str, symbols))
                }
            })
            .collect();

        let mut count = 0;
        let mut trie_root = TrieNode::default();
        let mut symbols_by_path = HashMap::with_capacity(parsed.len() * 3);
        let mut file_symbols = HashMap::with_capacity(parsed.len());

        for (rel_str, symbols) in parsed {
            for sym in &symbols {
                let sym_path = format!("{}::{}", rel_str, sym.name);
                let entry = SymbolEntry {
                    symbol_path: sym_path.clone(),
                    file_path: rel_str.clone(),
                    name: sym.name.clone(),
                    kind: sym.kind,
                    signature: sym.signature.clone(),
                    line_start: sym.line_start,
                    line_end: sym.line_end,
                    byte_start: sym.byte_start,
                    byte_end: sym.byte_end,
                    decorators: sym.decorators.clone(),
                };
                Self::insert_in_node(&mut trie_root, &sym_path, entry.clone());
                symbols_by_path.insert(sym_path, entry);
                count += 1;
            }
            file_symbols.insert(rel_str, symbols);
        }

        *self.root.write().unwrap() = trie_root;
        *self.symbols_by_path.write().unwrap() = symbols_by_path;
        *self.file_symbols.write().unwrap() = file_symbols;

        count
    }

    /// Exact symbol path lookup (0.00% hallucination guarantee)
    pub fn find_exact(&self, symbol_path: &str) -> Option<SymbolEntry> {
        let map = self.symbols_by_path.read().unwrap();
        let normalized = symbol_path.replace('\\', "/");
        if let Some(entry) = map.get(&normalized) {
            return Some(entry.clone());
        }

        // Try matching if user passed short name or suffix
        for (k, v) in map.iter() {
            if k.ends_with(&format!("::{}", normalized)) || k == &normalized {
                return Some(v.clone());
            }
        }

        None
    }

    /// Prefix search on symbol path
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<SymbolEntry> {
        let map = self.symbols_by_path.read().unwrap();
        let norm = prefix.replace('\\', "/");
        map.iter()
            .filter(|(k, _)| k.starts_with(&norm) || k.contains(&norm))
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Extracts the exact source code slice for a symbol from disk
    pub fn extract_symbol_code(&self, symbol_path: &str) -> Option<(SymbolEntry, String)> {
        if let Some(entry) = self.find_exact(symbol_path) {
            let abs_path = self.workspace_root.join(&entry.file_path);
            let content = fs::read_to_string(abs_path).ok()?;

            if entry.byte_end > entry.byte_start {
                if let Some(code_slice) = content.get(entry.byte_start..entry.byte_end) {
                    return Some((entry, code_slice.to_string()));
                }
            }
            // Fallback to line range
            let lines: Vec<&str> = content.lines().collect();
            let start_idx = entry.line_start.saturating_sub(1);
            let end_idx = entry.line_end.min(lines.len());
            let slice = lines[start_idx..end_idx].join("\n");
            return Some((entry, slice));
        }

        // On-demand fast extraction for single file if symbol_path contains file and symbol (e.g. 'path/file.py::Symbol')
        let norm = symbol_path.replace('\\', "/");
        if let Some((file_part, sym_name)) = norm.split_once("::") {
            let abs_path = self.workspace_root.join(file_part);
            if abs_path.is_file() {
                if let Ok(content) = fs::read_to_string(&abs_path) {
                    let symbols = AstOutlineParser::parse_file(&abs_path, &content);
                    for sym in symbols {
                        if sym.name == sym_name || sym.name.ends_with(sym_name) {
                            let entry = SymbolEntry {
                                symbol_path: format!("{}::{}", file_part, sym.name),
                                file_path: file_part.to_string(),
                                name: sym.name.clone(),
                                kind: sym.kind,
                                signature: sym.signature.clone(),
                                line_start: sym.line_start,
                                line_end: sym.line_end,
                                byte_start: sym.byte_start,
                                byte_end: sym.byte_end,
                                decorators: sym.decorators.clone(),
                            };
                            let slice = if entry.byte_end > entry.byte_start {
                                content.get(entry.byte_start..entry.byte_end).unwrap_or("").to_string()
                            } else {
                                let lines: Vec<&str> = content.lines().collect();
                                let s = entry.line_start.saturating_sub(1);
                                let e = entry.line_end.min(lines.len());
                                lines[s..e].join("\n")
                            };
                            return Some((entry, slice));
                        }
                    }
                }
            }
        }

        None
    }

    /// Returns Code Table of Contents outline for a file or directory
    pub fn get_outline(&self, path_filter: Option<&str>) -> Vec<(String, Vec<CodeSymbol>)> {
        let file_symbols = self.file_symbols.read().unwrap();
        let mut results = Vec::new();

        for (file, symbols) in file_symbols.iter() {
            if let Some(filter) = path_filter {
                let norm_filter = filter.replace('\\', "/");
                if !file.contains(&norm_filter) {
                    continue;
                }
            }
            results.push((file.clone(), symbols.clone()));
        }

        results.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    /// Fuzzy and subword search across symbol names, signatures, and file paths
    pub fn find_symbols_by_fuzzy_query(
        &self,
        query: &str,
        path_filter: Option<&str>,
        limit: usize,
    ) -> Vec<(SymbolEntry, f32)> {
        let map = self.symbols_by_path.read().unwrap();
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|t| !t.is_empty())
            .collect();

        if terms.is_empty() {
            return Vec::new();
        }

        let norm_filter = path_filter.map(|f| f.replace('\\', "/").to_lowercase());

        let mut scored: Vec<(SymbolEntry, f32)> = map
            .values()
            .filter(|entry| {
                if let Some(ref filter) = norm_filter {
                    entry.file_path.to_lowercase().contains(filter)
                } else {
                    true
                }
            })
            .filter_map(|entry| {
                let name_lower = entry.name.to_lowercase();
                let sig_lower = entry.signature.to_lowercase();
                let path_lower = entry.file_path.to_lowercase();

                let mut score: f32 = 0.0;
                let mut matched_terms = 0;

                for term in &terms {
                    let mut term_matched = false;
                    if name_lower == *term {
                        score += 15.0;
                        term_matched = true;
                    } else if name_lower.contains(term) {
                        score += 8.0;
                        term_matched = true;
                    }

                    if sig_lower.contains(term) {
                        score += 3.0;
                        term_matched = true;
                    }

                    if path_lower.contains(term) {
                        score += 1.5;
                        term_matched = true;
                    }

                    if term_matched {
                        matched_terms += 1;
                    }
                }

                if matched_terms > 0 {
                    if matched_terms == terms.len() {
                        score += 5.0;
                    }
                    Some((entry.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    /// Extracts a symbol's code with optional top imports and padding lines for surgical 1-shot editing
    pub fn extract_symbol_with_context(
        &self,
        symbol_path: &str,
        include_imports: bool,
        padding_lines: usize,
    ) -> Option<SymbolContextResult> {
        let entry = self.find_exact(symbol_path)?;
        let abs_path = self.workspace_root.join(&entry.file_path);
        let content = fs::read_to_string(&abs_path).ok()?;
        let lines: Vec<&str> = content.lines().collect();
        let total_file_lines = lines.len();

        let imports_header = if include_imports {
            let mut imp_lines = Vec::new();
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("import ")
                    || trimmed.starts_with("from ")
                    || trimmed.starts_with("use ")
                    || trimmed.starts_with("package ")
                    || trimmed.starts_with("require(")
                    || trimmed.starts_with("//")
                    || trimmed.starts_with("#[")
                    || trimmed.starts_with("/*")
                    || trimmed.is_empty()
                {
                    imp_lines.push(*line);
                } else {
                    break;
                }
            }
            if !imp_lines.is_empty() {
                Some(imp_lines.join("\n"))
            } else {
                None
            }
        } else {
            None
        };

        let start_line_0 = entry.line_start.saturating_sub(1);
        let end_line_0 = entry.line_end.min(lines.len());

        let padded_start = start_line_0.saturating_sub(padding_lines);
        let padded_end = (end_line_0 + padding_lines).min(lines.len());

        let code = if padding_lines == 0 && entry.byte_end > entry.byte_start {
            content
                .get(entry.byte_start..entry.byte_end)
                .map(|s| s.to_string())
                .unwrap_or_else(|| lines[start_line_0..end_line_0].join("\n"))
        } else {
            lines[padded_start..padded_end].join("\n")
        };

        Some(SymbolContextResult {
            entry,
            code,
            imports_header,
            line_start_with_padding: padded_start + 1,
            line_end_with_padding: padded_end,
            total_file_lines,
        })
    }
}
