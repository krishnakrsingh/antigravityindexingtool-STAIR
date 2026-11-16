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
    #[serde(default)]
    pub decorators: Vec<String>,
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

    /// Compute 0-based byte offset for the start of each line directly from content pointer slices
    pub fn compute_line_offsets(content: &str, lines: &[&str]) -> Vec<usize> {
        if lines.is_empty() {
            return vec![0];
        }
        let base = content.as_ptr() as usize;
        let mut offsets = Vec::with_capacity(lines.len() + 1);
        for line in lines {
            offsets.push(line.as_ptr() as usize - base);
        }
        offsets.push(content.len());
        offsets
    }

    /// Finds the end line (0-based) of a brace-enclosed block starting at `start_line`.
    /// When start_line contains function/method signatures, ensures brace counting
    /// only tracks the function body block (after `=> {`, `) {`, or return type),
    /// skipping any inline parameter type annotations containing braces (e.g. `viewProps: { ... }`)
    /// and any type-level braces in return types (e.g. `Promise<{ sort_order: number }>` or `: { x: number }`).
    pub fn find_brace_block_end(lines: &[&str], start_line: usize) -> usize {
        let mut brace_depth: i32 = 0;
        let mut found_body_brace = false;

        // Determine if there is a parameter list `(` or arrow `=>` before the first `{`
        let mut has_signature_parens = false;
        let mut has_arrow = false;
        'check_fn: for j in start_line..lines.len() {
            let l = lines[j].trim();
            for (idx, ch) in l.chars().enumerate() {
                if ch == '{' {
                    break 'check_fn;
                }
                if ch == '(' {
                    has_signature_parens = true;
                }
                if ch == '=' && idx + 1 < l.len() && l.as_bytes()[idx + 1] == b'>' {
                    has_arrow = true;
                }
            }
        }
        let is_function_like = has_signature_parens || has_arrow;

        let mut paren_depth: i32 = 0;
        let mut seen_closing_paren = false;
        let mut seen_arrow = false;
        let mut in_return_type = false;
        let mut angle_depth: i32 = 0;
        let mut return_type_brace_depth: i32 = 0;
        let mut seen_type_identifier = false;
        let mut in_quote = false;
        let mut quote_char = ' ';
        let mut in_block_comment = false;

        for j in start_line..lines.len() {
            let line = lines[j];
            let chars: Vec<char> = line.chars().collect();
            let mut k = 0;

            while k < chars.len() {
                let ch = chars[k];

                if in_block_comment {
                    if ch == '*' && k + 1 < chars.len() && chars[k + 1] == '/' {
                        in_block_comment = false;
                        k += 1;
                    }
                    k += 1;
                    continue;
                }

                if in_quote {
                    if ch == '\\' {
                        k += 1; // skip escaped char
                    } else if ch == quote_char {
                        in_quote = false;
                    }
                    k += 1;
                    continue;
                }

                // Outside quotes and comments
                if ch == '/' && k + 1 < chars.len() {
                    if chars[k + 1] == '/' {
                        break; // line comment, stop scanning this line
                    } else if chars[k + 1] == '*' {
                        in_block_comment = true;
                        k += 1;
                        k += 1;
                        continue;
                    }
                }

                if ch == '"' || ch == '\'' || ch == '`' {
                    in_quote = true;
                    quote_char = ch;
                    k += 1;
                    continue;
                }

                if !found_body_brace && is_function_like {
                    if ch == '(' {
                        paren_depth += 1;
                    } else if ch == ')' {
                        paren_depth = paren_depth.saturating_sub(1);
                        if paren_depth == 0 {
                            seen_closing_paren = true;
                        }
                    } else if ch == '=' && k + 1 < chars.len() && chars[k + 1] == '>' {
                        seen_arrow = true;
                        k += 1; // skip '>'
                    } else if paren_depth == 0 {
                        if seen_arrow {
                            if ch == '{' {
                                found_body_brace = true;
                                brace_depth = 1;
                            }
                        } else if seen_closing_paren {
                            if ch == ':' || (ch == '-' && k + 1 < chars.len() && chars[k + 1] == '>') {
                                in_return_type = true;
                                seen_type_identifier = false;
                                if ch == '-' {
                                    k += 1;
                                }
                            } else if in_return_type {
                                if ch == '<' {
                                    angle_depth += 1;
                                } else if ch == '>' {
                                    angle_depth = angle_depth.saturating_sub(1);
                                } else if ch == '{' {
                                    if angle_depth > 0 || !seen_type_identifier {
                                        return_type_brace_depth += 1;
                                    } else {
                                        // Body brace after type identifier/generic
                                        found_body_brace = true;
                                        brace_depth = 1;
                                    }
                                } else if ch == '}' && return_type_brace_depth > 0 {
                                    return_type_brace_depth -= 1;
                                    if return_type_brace_depth == 0 && angle_depth == 0 {
                                        seen_type_identifier = true;
                                    }
                                } else if ch.is_alphanumeric() || ch == '_' {
                                    seen_type_identifier = true;
                                } else if ch == '|' || ch == '&' {
                                    seen_type_identifier = false;
                                }
                            } else if ch == '{' {
                                found_body_brace = true;
                                brace_depth = 1;
                            }
                        }
                    }
                } else if ch == '{' {
                    brace_depth += 1;
                    found_body_brace = true;
                } else if ch == '}' {
                    brace_depth -= 1;
                    if found_body_brace && brace_depth == 0 {
                        return j;
                    }
                } else if ch == ';' && !found_body_brace && paren_depth == 0 && angle_depth == 0 {
                    // Semicolon terminated signature or statement without a body block
                    return j;
                }

                k += 1;
            }

            if found_body_brace && brace_depth == 0 {
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

        let offsets = Self::compute_line_offsets(content, &lines);
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
                        decorators: Vec::new(),
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
                        decorators: Vec::new(),
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
                        decorators: Vec::new(),
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
                    decorators: Vec::new(),
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
                                decorators: Vec::new(),
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
                        decorators: Vec::new(),
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

        let offsets = Self::compute_line_offsets(content, &lines);
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') {
                i += 1;
                continue;
            }

            // Top-level decorator capture
            let mut top_decorators: Vec<String> = Vec::new();
            let top_dec_start = i;
            if line.starts_with('@') {
                let mut d_paren: i32 = 0;
                let mut d_bracket: i32 = 0;
                let mut d_brace: i32 = 0;
                let mut cur_dec = String::new();

                while i < lines.len() {
                    let cur = lines[i].trim();
                    if cur.is_empty() || cur.starts_with("//") {
                        i += 1;
                        continue;
                    }
                    for ch in cur.chars() {
                        if ch == '(' { d_paren += 1; }
                        else if ch == ')' { d_paren = d_paren.saturating_sub(1); }
                        else if ch == '[' { d_bracket += 1; }
                        else if ch == ']' { d_bracket = d_bracket.saturating_sub(1); }
                        else if ch == '{' { d_brace += 1; }
                        else if ch == '}' { d_brace = d_brace.saturating_sub(1); }
                    }
                    if !cur_dec.is_empty() {
                        cur_dec.push(' ');
                    }
                    cur_dec.push_str(cur);

                    if d_paren == 0 && d_bracket == 0 && d_brace == 0 {
                        top_decorators.push(cur_dec.clone());
                        cur_dec.clear();
                        let mut next_k = i + 1;
                        while next_k < lines.len() {
                            let nl = lines[next_k].trim();
                            if nl.is_empty() || nl.starts_with("//") || nl.starts_with("/*") || nl.starts_with('*') {
                                next_k += 1;
                            } else {
                                break;
                            }
                        }
                        if next_k < lines.len() && lines[next_k].trim().starts_with('@') {
                            i = next_k;
                            continue;
                        }
                        i = next_k;
                        break;
                    }
                    i += 1;
                }
            }

            if i >= lines.len() {
                break;
            }

            let def_line = lines[i].trim();

            // Class matching
            if def_line.starts_with("class ")
                || def_line.starts_with("export class ")
                || def_line.starts_with("export default class ")
                || def_line.starts_with("abstract class ")
                || def_line.starts_with("export abstract class ")
            {
                let parts: Vec<&str> = def_line.split_whitespace().collect();
                let class_pos = parts.iter().position(|&p| p == "class").unwrap_or(0);
                if let Some(raw_name) = parts.get(class_pos + 1) {
                    let class_name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[top_dec_start];
                    let b_end = offsets[end_line + 1].min(content.len());

                    let mut class_symbol = CodeSymbol {
                        name: class_name.to_string(),
                        kind: SymbolKind::Class,
                        signature: def_line.to_string(),
                        line_start: top_dec_start + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators: top_decorators,
                    };

                    // Parse methods inside class
                    let mut method_idx = i + 1;
                    while method_idx < end_line {
                        let m_line = lines[method_idx].trim();
                        if m_line.is_empty()
                            || m_line.starts_with("//")
                            || m_line.starts_with("/*")
                            || m_line.starts_with('*')
                            || m_line.starts_with("if ")
                            || m_line.starts_with("if(")
                            || m_line.starts_with("for ")
                            || m_line.starts_with("for(")
                            || m_line.starts_with("while ")
                            || m_line.starts_with("while(")
                            || m_line.starts_with("switch ")
                            || m_line.starts_with("catch ")
                            || m_line.starts_with("return ")
                            || m_line.starts_with("super.")
                            || m_line.starts_with("constructor")
                        {
                            method_idx += 1;
                            continue;
                        }

                        // Collect decorators before class method/property
                        let mut m_decorators: Vec<String> = Vec::new();
                        let m_dec_start = method_idx;
                        if m_line.starts_with('@') {
                            let mut d_paren: i32 = 0;
                            let mut d_bracket: i32 = 0;
                            let mut d_brace: i32 = 0;
                            let mut cur_dec = String::new();

                            while method_idx < end_line {
                                let cur = lines[method_idx].trim();
                                if cur.is_empty() || cur.starts_with("//") {
                                    method_idx += 1;
                                    continue;
                                }
                                for ch in cur.chars() {
                                    if ch == '(' { d_paren += 1; }
                                    else if ch == ')' { d_paren = d_paren.saturating_sub(1); }
                                    else if ch == '[' { d_bracket += 1; }
                                    else if ch == ']' { d_bracket = d_bracket.saturating_sub(1); }
                                    else if ch == '{' { d_brace += 1; }
                                    else if ch == '}' { d_brace = d_brace.saturating_sub(1); }
                                }
                                if !cur_dec.is_empty() {
                                    cur_dec.push(' ');
                                }
                                cur_dec.push_str(cur);

                                if d_paren == 0 && d_bracket == 0 && d_brace == 0 {
                                    m_decorators.push(cur_dec.clone());
                                    cur_dec.clear();
                                    let mut next_k = method_idx + 1;
                                    while next_k < end_line {
                                        let nl = lines[next_k].trim();
                                        if nl.is_empty() || nl.starts_with("//") || nl.starts_with("/*") || nl.starts_with('*') {
                                            next_k += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    if next_k < end_line && lines[next_k].trim().starts_with('@') {
                                        method_idx = next_k;
                                        continue;
                                    }
                                    method_idx = next_k;
                                    break;
                                }
                                method_idx += 1;
                            }
                        }

                        if method_idx >= end_line {
                            break;
                        }

                        let target_m_line = lines[method_idx].trim();
                        if target_m_line.is_empty()
                            || target_m_line.starts_with("//")
                            || target_m_line.starts_with("constructor")
                        {
                            method_idx += 1;
                            continue;
                        }

                        // Check for arrow function property (e.g. `setSection = (...) => {` or `fetchProjects = async (...) => {`)
                        let is_arrow_property = if target_m_line.contains('=') && !target_m_line.contains("==") {
                            let mut found_arrow = false;
                            for k in method_idx..end_line {
                                let l = lines[k];
                                if l.contains("=>") {
                                    found_arrow = true;
                                    break;
                                }
                                if l.contains(';') && !l.contains('(') {
                                    break;
                                }
                            }
                            found_arrow
                        } else {
                            false
                        };

                        if is_arrow_property {
                            let left = target_m_line.split('=').next().unwrap_or("").trim();
                            let name_part = left.split(':').next().unwrap_or(left).trim();
                            let raw_name = name_part.split_whitespace().last().unwrap_or("").trim();
                            let m_name = raw_name.trim_start_matches('*');
                            if !m_name.is_empty() && m_name != "constructor" && !m_name.starts_with("super") {
                                let m_end = Self::find_brace_block_end(&lines, method_idx);
                                let m_bstart = offsets[m_dec_start];
                                let m_bend = offsets[m_end + 1].min(content.len());

                                let qual = format!("{}::{}", class_name, m_name);
                                let method_sym = CodeSymbol {
                                    name: qual,
                                    kind: SymbolKind::Method,
                                    signature: target_m_line.to_string(),
                                    line_start: m_dec_start + 1,
                                    line_end: m_end + 1,
                                    byte_start: m_bstart,
                                    byte_end: m_bend,
                                    children: Vec::new(),
                                    decorators: m_decorators,
                                };
                                class_symbol.children.push(method_sym.clone());
                                symbols.push(method_sym);
                                method_idx = m_end + 1;
                                continue;
                            }
                        }

                        // Check for standard method pattern: [modifier] [async] methodName(...)
                        // Scan ahead up to 5 lines if signature spans multiple lines before `(`
                        let mut m_sig_idx = method_idx;
                        let mut found_paren = false;
                        for k in method_idx..lines.len().min(method_idx + 5) {
                            let l = lines[k].trim();
                            if l.contains('(') {
                                m_sig_idx = k;
                                found_paren = true;
                                break;
                            }
                            if l.contains(';') || l.contains('{') {
                                break;
                            }
                        }

                        let is_method = if found_paren {
                            let mut found_brace = false;
                            for k in m_sig_idx..end_line {
                                let l = lines[k];
                                if l.contains('{') {
                                    found_brace = true;
                                    break;
                                }
                                if l.contains(';') && !l.contains('(') {
                                    break;
                                }
                            }
                            found_brace
                        } else {
                            false
                        };

                        if is_method {
                            let m_parts: Vec<&str> = lines[m_sig_idx].split('(').collect();
                            if let Some(prefix) = m_parts.first() {
                                let words: Vec<&str> = prefix.split_whitespace().collect();
                                if let Some(raw_mname) = words.last() {
                                    let m_name = raw_mname.trim_start_matches('*');
                                    if !m_name.is_empty() && m_name != "constructor" && !m_name.starts_with("super") {
                                        let m_end = Self::find_brace_block_end(&lines, method_idx);
                                        let m_bstart = offsets[m_dec_start];
                                        let m_bend = offsets[m_end + 1].min(content.len());

                                        let qual = format!("{}::{}", class_name, m_name);
                                        let method_sym = CodeSymbol {
                                            name: qual,
                                            kind: SymbolKind::Method,
                                            signature: lines[method_idx].trim().to_string(),
                                            line_start: m_dec_start + 1,
                                            line_end: m_end + 1,
                                            byte_start: m_bstart,
                                            byte_end: m_bend,
                                            children: Vec::new(),
                                            decorators: m_decorators,
                                        };
                                        class_symbol.children.push(method_sym.clone());
                                        symbols.push(method_sym);
                                        method_idx = m_end + 1;
                                        continue;
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
            } else if def_line.starts_with("interface ") || def_line.starts_with("export interface ") {
                let parts: Vec<&str> = def_line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "interface").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('{').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[top_dec_start];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Interface,
                        signature: def_line.to_string(),
                        line_start: top_dec_start + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators: top_decorators,
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if def_line.starts_with("type ") || def_line.starts_with("export type ") {
                let parts: Vec<&str> = def_line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "type").unwrap_or(0);
                if let Some(raw_name) = parts.get(pos + 1) {
                    let name = raw_name.trim_end_matches('=').trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[top_dec_start];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::TypeAlias,
                        signature: def_line.to_string(),
                        line_start: top_dec_start + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators: top_decorators,
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if def_line.starts_with("function ")
                || def_line.starts_with("export function ")
                || def_line.starts_with("export async function ")
                || def_line.starts_with("async function ")
                || def_line.starts_with("export default function ")
                || def_line.starts_with("export default async function ")
            {
                let parts: Vec<&str> = def_line.split_whitespace().collect();
                let pos = parts.iter().position(|&p| p == "function").unwrap_or(0);
                let raw_name_opt = parts.get(pos + 1).copied().or_else(|| {
                    if i + 1 < lines.len() {
                        let next_l = lines[i + 1].trim();
                        next_l.split_whitespace().next()
                    } else {
                        None
                    }
                });

                if let Some(raw_name) = raw_name_opt {
                    let name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    let end_line = Self::find_brace_block_end(&lines, i);
                    let b_start = offsets[top_dec_start];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        signature: def_line.to_string(),
                        line_start: top_dec_start + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators: top_decorators,
                    });
                    i = end_line + 1;
                    continue;
                }
            } else if (def_line.starts_with("export const ") || def_line.starts_with("const ") || def_line.starts_with("export let ") || def_line.starts_with("let "))
                && def_line.contains('=')
            {
                let is_arrow = {
                    let mut found_arrow = false;
                    for k in i..lines.len().min(i + 30) {
                        let l = lines[k];
                        if l.contains("=>") {
                            found_arrow = true;
                            break;
                        }
                        if l.contains(';') && !l.contains('(') {
                            break;
                        }
                    }
                    found_arrow
                };

                if is_arrow {
                    let left = def_line.split('=').next().unwrap_or("").trim();
                    let name_part = left.split(':').next().unwrap_or(left).trim();
                    let fn_name = name_part.split_whitespace().last().unwrap_or("").trim();
                    if !fn_name.is_empty() {
                        let end_line = Self::find_brace_block_end(&lines, i);
                        let b_start = offsets[top_dec_start];
                        let b_end = offsets[end_line + 1].min(content.len());

                        symbols.push(CodeSymbol {
                            name: fn_name.to_string(),
                            kind: SymbolKind::Function,
                            signature: def_line.to_string(),
                            line_start: top_dec_start + 1,
                            line_end: end_line + 1,
                            byte_start: b_start,
                            byte_end: b_end,
                            children: Vec::new(),
                            decorators: top_decorators,
                        });
                        i = end_line + 1;
                        continue;
                    }
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

        let offsets = Self::compute_line_offsets(content, &lines);
        let mut i = 0;

        fn line_indent(l: &str) -> usize {
            l.len() - l.trim_start().len()
        }

        fn find_python_block_end(lines: &[&str], start: usize, block_indent: usize) -> usize {
            // 1. Skip past multi-line def/class signature until ':' outside parens/brackets
            let mut body_start = start;
            let mut p_depth: i32 = 0;
            let mut b_depth: i32 = 0;
            for j in start..lines.len() {
                let l = lines[j];
                for ch in l.chars() {
                    if ch == '(' { p_depth += 1; }
                    else if ch == ')' { p_depth = p_depth.saturating_sub(1); }
                    else if ch == '[' { b_depth += 1; }
                    else if ch == ']' { b_depth = b_depth.saturating_sub(1); }
                }
                if p_depth == 0 && b_depth == 0 && l.trim_end().ends_with(':') {
                    body_start = j + 1;
                    break;
                }
            }

            let mut end = body_start.saturating_sub(1).max(start);
            for j in body_start..lines.len() {
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

            // Check if there are decorators before a class or def
            let mut decorators: Vec<String> = Vec::new();
            let dec_start_idx = i;

            if trimmed.starts_with('@') {
                let mut d_paren: i32 = 0;
                let mut d_bracket: i32 = 0;
                let mut d_brace: i32 = 0;
                let mut cur_dec = String::new();

                while i < lines.len() {
                    let cur = lines[i].trim();
                    if cur.is_empty() || cur.starts_with('#') {
                        i += 1;
                        continue;
                    }
                    for ch in cur.chars() {
                        if ch == '(' { d_paren += 1; }
                        else if ch == ')' { d_paren = d_paren.saturating_sub(1); }
                        else if ch == '[' { d_bracket += 1; }
                        else if ch == ']' { d_bracket = d_bracket.saturating_sub(1); }
                        else if ch == '{' { d_brace += 1; }
                        else if ch == '}' { d_brace = d_brace.saturating_sub(1); }
                    }
                    if !cur_dec.is_empty() {
                        cur_dec.push(' ');
                    }
                    cur_dec.push_str(cur);

                    if d_paren == 0 && d_bracket == 0 && d_brace == 0 {
                        decorators.push(cur_dec.clone());
                        cur_dec.clear();
                        let mut next_k = i + 1;
                        while next_k < lines.len() {
                            let nl = lines[next_k].trim();
                            if nl.is_empty() || nl.starts_with('#') {
                                next_k += 1;
                            } else {
                                break;
                            }
                        }
                        if next_k < lines.len() && lines[next_k].trim().starts_with('@') {
                            i = next_k;
                            continue;
                        }
                        i = next_k;
                        break;
                    }
                    i += 1;
                }
            }

            if i >= lines.len() {
                break;
            }

            let def_line = lines[i];
            let def_trimmed = def_line.trim();
            let base_indent = line_indent(def_line);

            if def_trimmed.starts_with("class ") {
                let parts: Vec<&str> = def_trimmed.split_whitespace().collect();
                if let Some(raw_name) = parts.get(1) {
                    let class_name = raw_name.split('(').next().unwrap_or(raw_name).trim_end_matches(':').trim();
                    let end_line = find_python_block_end(&lines, i, base_indent);
                    let b_start = offsets[dec_start_idx];
                    let b_end = offsets[end_line + 1].min(content.len());

                    let mut class_symbol = CodeSymbol {
                        name: class_name.to_string(),
                        kind: SymbolKind::Class,
                        signature: def_trimmed.to_string(),
                        line_start: dec_start_idx + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators: decorators.clone(),
                    };

                    // Parse methods inside class
                    let mut m_idx = i + 1;
                    while m_idx <= end_line {
                        let m_line = lines[m_idx];
                        let m_trimmed = m_line.trim();
                        if m_trimmed.is_empty() || m_trimmed.starts_with('#') {
                            m_idx += 1;
                            continue;
                        }

                        let mut m_decorators: Vec<String> = Vec::new();
                        let m_dec_start = m_idx;

                        if m_trimmed.starts_with('@') {
                            let mut d_paren: i32 = 0;
                            let mut d_bracket: i32 = 0;
                            let mut d_brace: i32 = 0;
                            let mut cur_dec = String::new();

                            while m_idx <= end_line {
                                let cur = lines[m_idx].trim();
                                if cur.is_empty() || cur.starts_with('#') {
                                    m_idx += 1;
                                    continue;
                                }
                                for ch in cur.chars() {
                                    if ch == '(' { d_paren += 1; }
                                    else if ch == ')' { d_paren = d_paren.saturating_sub(1); }
                                    else if ch == '[' { d_bracket += 1; }
                                    else if ch == ']' { d_bracket = d_bracket.saturating_sub(1); }
                                    else if ch == '{' { d_brace += 1; }
                                    else if ch == '}' { d_brace = d_brace.saturating_sub(1); }
                                }
                                if !cur_dec.is_empty() {
                                    cur_dec.push(' ');
                                }
                                cur_dec.push_str(cur);

                                if d_paren == 0 && d_bracket == 0 && d_brace == 0 {
                                    m_decorators.push(cur_dec.clone());
                                    cur_dec.clear();
                                    let mut next_k = m_idx + 1;
                                    while next_k <= end_line {
                                        let nl = lines[next_k].trim();
                                        if nl.is_empty() || nl.starts_with('#') {
                                            next_k += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    if next_k <= end_line && lines[next_k].trim().starts_with('@') {
                                        m_idx = next_k;
                                        continue;
                                    }
                                    m_idx = next_k;
                                    break;
                                }
                                m_idx += 1;
                            }
                        }

                        if m_idx > end_line {
                            break;
                        }

                        let target_m_line = lines[m_idx];
                        let target_m_trimmed = target_m_line.trim();

                        if target_m_trimmed.starts_with("def ") || target_m_trimmed.starts_with("async def ") {
                            let m_indent = line_indent(target_m_line);
                            if m_indent > base_indent {
                                let m_parts: Vec<&str> = target_m_trimmed.split_whitespace().collect();
                                let def_pos = m_parts.iter().position(|&p| p == "def").unwrap_or(0);
                                if let Some(m_name_raw) = m_parts.get(def_pos + 1) {
                                    let m_name = m_name_raw.split('(').next().unwrap_or(m_name_raw).trim();
                                    let m_end = find_python_block_end(&lines, m_idx, m_indent);
                                    let m_bstart = offsets[m_dec_start];
                                    let m_bend = offsets[m_end + 1].min(content.len());

                                    let qual = format!("{}::{}", class_name, m_name);
                                    let method_symbol = CodeSymbol {
                                        name: qual,
                                        kind: SymbolKind::Method,
                                        signature: target_m_trimmed.to_string(),
                                        line_start: m_dec_start + 1,
                                        line_end: m_end + 1,
                                        byte_start: m_bstart,
                                        byte_end: m_bend,
                                        children: Vec::new(),
                                        decorators: m_decorators,
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
            } else if def_trimmed.starts_with("def ") || def_trimmed.starts_with("async def ") {
                let parts: Vec<&str> = def_trimmed.split_whitespace().collect();
                let def_pos = parts.iter().position(|&p| p == "def").unwrap_or(0);
                if let Some(raw_name) = parts.get(def_pos + 1) {
                    let fn_name = raw_name.split('(').next().unwrap_or(raw_name).trim();
                    let end_line = find_python_block_end(&lines, i, base_indent);
                    let b_start = offsets[dec_start_idx];
                    let b_end = offsets[end_line + 1].min(content.len());

                    symbols.push(CodeSymbol {
                        name: fn_name.to_string(),
                        kind: SymbolKind::Function,
                        signature: def_trimmed.to_string(),
                        line_start: dec_start_idx + 1,
                        line_end: end_line + 1,
                        byte_start: b_start,
                        byte_end: b_end,
                        children: Vec::new(),
                        decorators,
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

        let offsets = Self::compute_line_offsets(content, &lines);
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
                            decorators: Vec::new(),
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
                        decorators: Vec::new(),
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
                        decorators: Vec::new(),
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

        let offsets = Self::compute_line_offsets(content, &lines);

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
                    decorators: Vec::new(),
                });
            }
        }
        symbols
    }

    fn parse_generic(_content: &str) -> Vec<CodeSymbol> {
        Vec::new()
    }
}
