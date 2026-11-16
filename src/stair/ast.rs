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

    /// High-speed AST outline parser for Rust
    fn parse_rust(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut byte_offset = 0usize;
        let mut line_byte_offsets = Vec::with_capacity(lines.len());

        for line in &lines {
            line_byte_offsets.push(byte_offset);
            byte_offset += line.len() + 1; // +1 for newline
        }

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("//") || line.is_empty() {
                i += 1;
                continue;
            }

            // Rust struct / enum / trait / impl / fn / mod
            let symbol_kind_and_name = if line.starts_with("pub struct ") || line.starts_with("struct ") {
                let name = line.split_whitespace().nth(if line.starts_with("pub ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Struct, n.trim_end_matches('{').trim_end_matches(';')))
            } else if line.starts_with("pub enum ") || line.starts_with("enum ") {
                let name = line.split_whitespace().nth(if line.starts_with("pub ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Enum, n.trim_end_matches('{')))
            } else if line.starts_with("pub trait ") || line.starts_with("trait ") {
                let name = line.split_whitespace().nth(if line.starts_with("pub ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Trait, n.trim_end_matches('{')))
            } else if line.starts_with("impl ") || line.starts_with("pub impl ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let name = parts.get(1).unwrap_or(&"impl");
                Some((SymbolKind::Class, *name))
            } else if line.starts_with("pub fn ") || line.starts_with("fn ") || line.starts_with("pub async fn ") || line.starts_with("async fn ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let fn_pos = parts.iter().position(|&p| p == "fn").unwrap_or(0);
                let name = parts.get(fn_pos + 1).map(|n| n.split('(').next().unwrap_or(n));
                name.map(|n| (SymbolKind::Function, n))
            } else if line.starts_with("pub mod ") || line.starts_with("mod ") {
                let name = line.split_whitespace().nth(if line.starts_with("pub ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Module, n.trim_end_matches('{').trim_end_matches(';')))
            } else {
                None
            };

            if let Some((kind, raw_name)) = symbol_kind_and_name {
                let name = raw_name.trim().to_string();
                let line_start = i + 1;
                let b_start = line_byte_offsets[i];

                // Find closing brace or semicolon
                let mut brace_depth = 0;
                let mut found_brace = false;
                let mut end_line = i;

                for j in i..lines.len() {
                    let cur_line = lines[j];
                    for ch in cur_line.chars() {
                        if ch == '{' {
                            brace_depth += 1;
                            found_brace = true;
                        } else if ch == '}' {
                            brace_depth -= 1;
                        }
                    }

                    if found_brace && brace_depth == 0 {
                        end_line = j;
                        break;
                    }
                    if !found_brace && cur_line.trim().ends_with(';') {
                        end_line = j;
                        break;
                    }
                    end_line = j;
                }

                let b_end = if end_line < line_byte_offsets.len() {
                    line_byte_offsets[end_line] + lines[end_line].len()
                } else {
                    content.len()
                };

                symbols.push(CodeSymbol {
                    name,
                    kind,
                    signature: line.to_string(),
                    line_start,
                    line_end: end_line + 1,
                    byte_start: b_start,
                    byte_end: b_end,
                    children: Vec::new(),
                });

                i = end_line + 1;
            } else {
                i += 1;
            }
        }

        symbols
    }

    /// AST outline parser for TypeScript / JavaScript
    fn parse_typescript(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut byte_offset = 0usize;
        let mut line_byte_offsets = Vec::with_capacity(lines.len());

        for line in &lines {
            line_byte_offsets.push(byte_offset);
            byte_offset += line.len() + 1;
        }

        for (i, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            let symbol_match = if trimmed.starts_with("class ") || trimmed.starts_with("export class ") || trimmed.starts_with("export default class ") {
                let name = trimmed.split_whitespace().nth(if trimmed.starts_with("export default ") { 3 } else if trimmed.starts_with("export ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Class, n.trim_end_matches('{')))
            } else if trimmed.starts_with("interface ") || trimmed.starts_with("export interface ") {
                let name = trimmed.split_whitespace().nth(if trimmed.starts_with("export ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::Interface, n.trim_end_matches('{')))
            } else if trimmed.starts_with("type ") || trimmed.starts_with("export type ") {
                let name = trimmed.split_whitespace().nth(if trimmed.starts_with("export ") { 2 } else { 1 });
                name.map(|n| (SymbolKind::TypeAlias, n.trim_end_matches('=')))
            } else if trimmed.starts_with("function ") || trimmed.starts_with("export function ") || trimmed.starts_with("export async function ") || trimmed.starts_with("async function ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let fn_pos = parts.iter().position(|&p| p == "function").unwrap_or(0);
                let name = parts.get(fn_pos + 1).map(|n| n.split('(').next().unwrap_or(n));
                name.map(|n| (SymbolKind::Function, n))
            } else {
                None
            };

            if let Some((kind, raw_name)) = symbol_match {
                let b_start = line_byte_offsets[i];
                let b_end = (b_start + line.len()).min(content.len());
                symbols.push(CodeSymbol {
                    name: raw_name.trim().to_string(),
                    kind,
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

    /// AST outline parser for Python
    fn parse_python(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut byte_offset = 0usize;
        let mut line_byte_offsets = Vec::with_capacity(lines.len());

        for line in &lines {
            line_byte_offsets.push(byte_offset);
            byte_offset += line.len() + 1;
        }

        for (i, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let def_pos = parts.iter().position(|&p| p == "def").unwrap_or(0);
                if let Some(name_part) = parts.get(def_pos + 1) {
                    let name = name_part.split('(').next().unwrap_or(name_part);
                    let b_start = line_byte_offsets[i];
                    let b_end = (b_start + line.len()).min(content.len());
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: i + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                }
            } else if trimmed.starts_with("class ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(name_part) = parts.get(1) {
                    let name = name_part.split('(').next().unwrap_or(name_part).trim_end_matches(':');
                    let b_start = line_byte_offsets[i];
                    let b_end = (b_start + line.len()).min(content.len());
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: i + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                    });
                }
            }
        }

        symbols
    }

    /// AST outline parser for Go
    fn parse_go(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("func ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(name_part) = parts.get(1) {
                    let name = name_part.split('(').next().unwrap_or(name_part);
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: i + 1,
                        byte_start: 0,
                        byte_end: 0,
                        children: Vec::new(),
                    });
                }
            } else if trimmed.starts_with("type ") && (trimmed.contains("struct") || trimmed.contains("interface")) {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if let Some(name) = parts.get(1) {
                    let kind = if trimmed.contains("interface") {
                        SymbolKind::Interface
                    } else {
                        SymbolKind::Struct
                    };
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind,
                        signature: trimmed.to_string(),
                        line_start: i + 1,
                        line_end: i + 1,
                        byte_start: 0,
                        byte_end: 0,
                        children: Vec::new(),
                    });
                }
            }
        }
        symbols
    }

    /// Outline parser for Markdown
    fn parse_markdown(content: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let title = trimmed.trim_start_matches('#').trim();
                symbols.push(CodeSymbol {
                    name: title.to_string(),
                    kind: SymbolKind::Section,
                    signature: trimmed.to_string(),
                    line_start: i + 1,
                    line_end: i + 1,
                    byte_start: 0,
                    byte_end: 0,
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
