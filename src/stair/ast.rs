use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Function,
    Method,
    Constructor,
    TypeAlias,
    Constant,
    Section, // for markdown/docs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub children: Vec<CodeSymbol>,
}

pub struct AstOutlineParser;

impl AstOutlineParser {
    pub fn parse_file(path: &Path, content: &str) -> Vec<CodeSymbol> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "rs" => Self::parse_rust(content),
            "ts" | "tsx" | "js" | "jsx" => Self::parse_typescript(content),
            "py" => Self::parse_python(content),
            "go" => Self::parse_go(content),
            "md" => Self::parse_markdown(content),
            _ => Self::parse_generic(content),
        }
    }

    /// Compute 0-based byte offset for the start of each line
    fn compute_line_offsets(lines: &[&str]) -> Vec<usize> {
        let mut offsets = Vec::with_capacity(lines.len() + 1);
        let mut curr = 0;
        for line in lines {
            offsets.push(curr);
            curr += line.len() + 1; // assumes \n (or approx for slice boundary)
        }
        offsets.push(curr);
        offsets
    }

    /// Finds the end line (0-based) of a brace-enclosed block starting at `start_line`
    fn find_brace_block_end(lines: &[&str], start_line: usize) -> usize {
        let mut brace_depth = 0;
        let mut found_brace = false;

        for j in start_line..lines.len() {
            let line = lines[j];
            let mut in_quote = false;
            let mut quote_char = ' ';
            let chars: Vec<char> = line.chars().collect();
            let mut k = 0;

            while k < chars.len() {
                let ch = chars[k];

                if !in_quote {
                    if ch == '/' && k + 1 < chars.len() && chars[k + 1] == '/' {
                        break; // line comment, stop scanning this line
                    }
                    if ch == '"' || ch == '\'' || ch == '`' {
                        in_quote = true;
                        quote_char = ch;
                    } else if ch == '{' {
                        brace_depth += 1;
                        found_brace = true;
                    } else if ch == '}' {
                        brace_depth -= 1;
                        if found_brace && brace_depth == 0 {
                            return j;
                        }
                    } else if ch == ';' && !found_brace && j == start_line {
                        // Semicolon terminated single-line statement
                        return j;
                    }
                } else {
                    if ch == '\\' {
                        k += 1; // skip escaped char
                    } else if ch == quote_char {
                        in_quote = false;
                    }
                }
                k += 1;
            }

            if found_brace && brace_depth == 0 {
                return j;
            }
        }

        // If block unclosed, return last line
        lines.len().saturating_sub(1)
    }

    /// High-speed AST outline parser for Rust with full impl method extraction
    pub fn parse_rust(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return symbols;
        }

        let offsets = Self::compute_line_offsets(&lines);
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("//") || line.starts_with("/*") || line.is_empty() || line.starts_with("#[") {
                i += 1;
                continue;
            }

            // Rust struct / enum / trait / type
            if line.starts_with("pub struct ") || line.starts_with("struct ") || line.starts_with("pub(crate) struct ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let struct_pos = parts.iter().position(|&p| p == "struct").unwrap_or(0);
                if let Some(raw_name) = parts.get(struct_pos + 1) {
                    let name = raw_name.trim_end_matches('{').trim_end_matches(';').trim_end_matches('(').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Struct,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("pub enum ") || line.starts_with("enum ") || line.starts_with("pub(crate) enum ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "enum").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Enum,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("pub trait ") || line.starts_with("trait ") || line.starts_with("pub(crate) trait ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "trait").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Trait,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("impl ") || line.starts_with("pub impl ") {
                // impl [Trait for] TargetType
                let parts: Vec<&str> = line.split_whitespace().collect();
                let target_type = if let Some(for_idx) = parts.iter().position(|&p| p == "for") {
                    parts.get(for_idx + 1).map(|s| s.trim_end_matches('{')).unwrap_or("impl")
                } else {
                    parts.get(1).map(|s| s.trim_end_matches('{')).unwrap_or("impl")
                };
                let clean_type = target_type.split('<').next().unwrap_or(target_type).trim();

                let impl_end = Self::find_brace_block_end(&lines, i);
                let b_start = offsets[i];
                let b_end = offsets[impl_end + 1].min(content.len());

                let mut impl_symbol = CodeSymbol {
                    name: clean_type.to_string(),
                    kind: SymbolKind::Class,
                    signature: line.to_string(),
                    line_start: i + 1,
                    line_end: impl_end + 1,
                    byte_start: b_start,
                    byte_end: b_end,
                    children: Vec::new(),
                };

                // Parse methods INSIDE the impl block!
                let mut method_idx = i + 1;
                while method_idx < impl_end {
                    let m_line = lines[method_idx].trim();
                    if m_line.starts_with("pub fn ")
                        || m_line.starts_with("fn ")
                        || m_line.starts_with("pub async fn ")
                        || m_line.starts_with("async fn ")
                        || m_line.starts_with("pub(crate) fn ")
                    {
                        let m_parts: Vec<&str> = m_line.split_whitespace().collect();
                        let fn_pos = m_parts.iter().position(|&p| p == "fn").unwrap_or(0);
                        if let Some(raw_fn_name) = m_parts.get(fn_pos + 1) {
                            let fn_name = raw_fn_name.split('(').next().unwrap_or(raw_fn_name).trim();
                            let m_end = Self::find_brace_block_end(&lines, method_idx);
                            let m_bstart = offsets[method_idx];
                            let m_bend = offsets[m_end + 1].min(content.len());

                            let qualified_name = format!("{}::{}", clean_type, fn_name);
                            let method_symbol = CodeSymbol {
                                name: qualified_name,
                                kind: SymbolKind::Method,
                                signature: m_line.to_string(),
                                line_start: method_idx + 1,
                                line_end: m_end + 1,
                                byte_start: m_bstart,
                                byte_end: m_bend,
                                children: Vec::new(),
                            };
                            impl_symbol.children.push(method_symbol.clone());
                            // Also push as top-level discoverable symbol
                            symbols.push(method_symbol);
                            method_idx = m_end + 1;
                            continue;
                        }
                    }
                    method_idx += 1;
                }

                symbols.push(impl_symbol);
                i = impl_end + 1;
                continue;
            } else if line.starts_with("pub fn ")
                || line.starts_with("fn ")
                || line.starts_with("pub async fn ")
                || line.starts_with("async fn ")
                || line.starts_with("pub(crate) fn ")
            {
                // Top-level function
                let parts: Vec<&str> = line.split_whitespace().collect();
                let fn_pos = parts.iter().position(|&p| p == "fn").unwrap_or(0);
                if let Some(raw_name) = parts.get(fn_pos + 1) {
                    let name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            }

            i += 1;
        }

        symbols
    }

    /// AST outline parser for TypeScript / JavaScript with full method and block extraction
    pub fn parse_typescript(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return symbols;
        }

        let offsets = Self::compute_line_offsets(&lines);
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') {
                i += 1;
                continue;
            }

            // Class matching
            if line.starts_with("class ")
                || line.starts_with("export class ")
                || line.starts_with("export default class ")
                || line.starts_with("abstract class ")
                || line.starts_with("export abstract class ")
            {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let class_pos = parts.iter().position(|&p| p == "class").unwrap_or(0);
                if let Some(raw_name) = parts.get(class_pos + 1) {
                    let class_name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    let mut class_symbol = CodeSymbol {
                        name: class_name.to_string(),
                        kind: SymbolKind::Class,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    };

                    // Parse methods inside class
                    let mut method_idx = i + 1;
                    while method_idx < end_line {
                        let m_line = lines[method_idx].trim();
                        if !m_line.is_empty() && !m_line.starts_with("//") && !m_line.starts_with("/*") {
                            // Check for method pattern: [modifier] [async] methodName(...)
                            let is_method = (m_line.contains('(') && (m_line.ends_with('{') || m_line.contains(") {")))
                                && !m_line.starts_with("if ")
                                && !m_line.starts_with("for ")
                                && !m_line.starts_with("while ")
                                && !m_line.starts_with("switch ")
                                && !m_line.starts_with("catch ");

                            if is_method {
                                let m_parts: Vec<&str> = m_line.split('(').collect();
                                if let Some(prefix) = m_parts.first() {
                                    let words: Vec<&str> = prefix.split_whitespace().collect();
                                    if let Some(raw_mname) = words.last() {
                                        let m_name = raw_mname.trim_start_matches('*');
                                        if !m_name.is_empty() && m_name != "constructor" && !m_name.starts_with("super") {
                                            let m_end = Self::find_brace_block_end(&lines, method_idx);
                                            let m_bstart = offsets[method_idx];
                                            let m_bend = offsets[m_end + 1].min(content.len());

                                            let qual = format!("{}::{}", class_name, m_name);
                                            let method_sym = CodeSymbol {
                                                name: qual,
                                                kind: SymbolKind::Method,
                                                signature: m_line.to_string(),
                                                line_start: method_idx + 1,
                                                line_end: m_end + 1,
                                                byte_start: m_bstart,
                                                byte_end: m_bend,
                                                children: Vec::new(),
                                            };
                                            class_symbol.children.push(method_sym.clone());
                                            symbols.push(method_sym);
                                            method_idx = m_end + 1;
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        method_idx += 1;
                    }

                    symbols.push(class_symbol);
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("interface ") || line.starts_with("export interface ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "interface").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Interface,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("type ") || line.starts_with("export type ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "type").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('=').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::TypeAlias,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("function ")
                || line.starts_with("export function ")
                || line.starts_with("export async function ")
                || line.starts_with("async function ")
                || line.starts_with("export default function ")
            {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "function").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            }

            i += 1;
        }

        symbols
    }

    /// AST outline parser for Python with indentation-aware function/method extraction
    pub fn parse_python(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return symbols;
        }

        let offsets = Self::compute_line_offsets(&lines);
        let mut i = 0;

        fn line_indent(l: &str) -> usize {
            l.len() - l.trim_start().len()
        }

        fn find_python_block_end(lines: &[&str], start: usize, block_indent: usize) -> usize {
            let mut end = start;
            for j in (start + 1)..lines.len() {
                let l = lines[j];
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let cur_indent = line_indent(l);
                if cur_indent <= block_indent {
                    return end;
                }
                end = j;
            }
            lines.len().saturating_sub(1)
        }

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                i += 1;
                continue;
            }

            let base_indent = line_indent(line);

            if trimmed.starts_with("class ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(raw_name) = parts.get(1) {
                    let class_name = raw_name.split('(').next().unwrap_or(raw_name).trim_end_matches(':').trim();
                    let end_line = find_python_block_end(&lines, i, base_indent);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    let mut class_symbol = CodeSymbol {
                        name: class_name.to_string(),
                        kind: SymbolKind::Class,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    };

                    // Parse methods inside class
                    let mut m_idx = i + 1;
                    while m_idx <= end_line {
                        let m_line = lines[m_idx];
                        let m_trimmed = m_line.trim();
                        if m_trimmed.starts_with("def ") || m_trimmed.starts_with("async def ") {
                            let m_indent = line_indent(m_line);
                            if m_indent > base_indent {
                                let m_parts: Vec<&str> = m_trimmed.split_whitespace().collect();
                                let def_pos = m_parts.iter().position(|&p| p == "def").unwrap_or(0);
                                if let Some(m_name_raw) = m_parts.get(def_pos + 1) {
                                    let m_name = m_name_raw.split('(').next().unwrap_or(m_name_raw).trim();
                                    let m_end = find_python_block_end(&lines, m_idx, m_indent);
                                    let m_bstart = offsets[m_idx];
                                    let m_bend = offsets[m_end + 1].min(content.len());

                                    let qual = format!("{}::{}", class_name, m_name);
                                    let method_symbol = CodeSymbol {
                                        name: qual,
                                        kind: SymbolKind::Method,
                                        signature: m_trimmed.to_string(),
                                        line_start: m_idx + 1,
                                        line_end: m_end + 1,
                                        byte_start: m_bstart,
                                        byte_end: m_bend,
                                        children: Vec::new(),
                                    };
                                    class_symbol.children.push(method_symbol.clone());
                                    symbols.push(method_symbol);
                                    m_idx = m_end + 1;
                                    continue;
                                }
                            }
                        }
                        m_idx += 1;
                    }

                    symbols.push(class_symbol);
                    i = end_line + 1;
                    continue;
                }
            } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let def_pos = parts.iter().position(|&p| p == "def").unwrap_or(0);
                if let Some(raw_name) = parts.get(def_pos + 1) {
                    let fn_name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    let end_line = find_python_block_end(&lines, i, base_indent);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: fn_name.to_string(),
                        kind: SymbolKind::Function,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            }

            i += 1;
        }

        symbols
    }

    /// AST outline parser for Go with method receiver resolution
    pub fn parse_go(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return symbols;
        }

        let offsets = Self::compute_line_offsets(&lines);
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            if line.starts_with("func ") {
                let end_line = Self::find_brace_block_end(&lines, i);
                let b_start = offsets[i];
                let b_end = offsets[end_line + 1].min(content.len());

                // Check if method on receiver: func (r *Type) Method(...)
                if line.starts_with("func (") {
                    let after_func = &line[5..];
                    if let Some(close_paren) = after_func.find(')') {
                        let receiver_part = &after_func[..close_paren];
                        let r_type = receiver_part
                            .split_whitespace()
                            .last()
                            .unwrap_or("")
                            .trim_start_matches('*');
                        let rest = after_func[close_paren + 1..].trim();
                        let method_name = rest.split('(').next().unwrap_or("").trim();

                        let qual = format!("{}::{}", r_type, method_name);
                        symbols.push(CodeSymbol {
                            name: qual,
                            kind: SymbolKind::Method,
                            signature: line.to_string(),
                            line_start: i + 1,
                            line_end: end_line + 1,
                            byte_start: b_start,
                            byte_end: b_end,
                            children: Vec::new(),
                        });
                        i = end_line + 1;
                        continue;
                    }
                }

                // Regular function
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(raw_name) = parts.get(1) {
                    let name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if line.starts_with("type ") && (line.contains("struct") || line.contains("interface")) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(raw_name) = parts.get(1) {
                    let kind = if line.contains("interface") {
                        SymbolKind::Interface
                    } else {
                        SymbolKind::Struct
                    };
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[i];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: raw_name.to_string(),
                        kind,
                        signature: line.to_string(),
                        line_start: i + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                    i = end_line + 1;
                    continue;
                }
            }

            i += 1;
        }

        symbols
    }

    /// Outline parser for Markdown
    pub fn parse_markdown(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return symbols;
        }

        let offsets = Self::compute_line_offsets(&lines);

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let title = trimmed.trim_start_matches('#').trim();
                let b_start = offsets[i];
                let b_end = offsets[i + 1].min(content.len());

                symbols.push(CodeSymbol {
                    name: title.to_string(),
                    kind: SymbolKind::Section,
                    signature: trimmed.to_string(),
                    line_start: i + 1,
                    line_end: i + 1,
                    byte_start: b_start,
                    byte_end: b_end,
                    children: Vec::new(),
                });
            }
        }
        symbols
    }

    fn parse_generic(_content: &str) -> Vec<CodeSymbol> {
        Vec::new()
    }
}
