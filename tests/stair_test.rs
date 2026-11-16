use antigravity_fast_core::{AstOutlineParser, SymbolKind, SymbolTrie};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_ast_outline_parsing_rust() {
    let code = r#"
pub struct UserSession {
    pub id: u64,
}

pub enum SessionState {
    Active,
    Expired,
}

pub fn validate_session(session: &UserSession) -> bool {
    session.id > 0
}
"#;

    let symbols = AstOutlineParser::parse_file(std::path::Path::new("session.rs"), code);
    assert_eq!(symbols.len(), 3);
    assert_eq!(symbols[0].name, "UserSession");
    assert_eq!(symbols[0].kind, SymbolKind::Struct);
    assert_eq!(symbols[1].name, "SessionState");
    assert_eq!(symbols[1].kind, SymbolKind::Enum);
    assert_eq!(symbols[2].name, "validate_session");
    assert_eq!(symbols[2].kind, SymbolKind::Function);
}

#[test]
fn test_symbol_trie_zero_hallucination_and_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("service.rs");
    let content = r#"pub struct TokenService {
    secret: String,
}

pub fn create_token() -> String {
    "xyz123".to_string()
}
"#;
    fs::write(&file_path, content).unwrap();

    let trie = SymbolTrie::new(root);
    let count = trie.build();
    assert_eq!(count, 2);

    // Exact lookup test
    let exact_path = "src/service.rs::TokenService";
    let entry = trie.find_exact(exact_path);
    assert!(entry.is_some(), "Exact symbol path must resolve");
    assert_eq!(entry.unwrap().name, "TokenService");

    // Non-existent symbol must return None (0.00% hallucination)
    let fake_path = "src/service.rs::NonExistentSymbol";
    assert!(trie.find_exact(fake_path).is_none(), "Must guarantee zero hallucination");

    // Extraction test
    let (sym_entry, code_slice) = trie.extract_symbol_code(exact_path).expect("Extraction failed");
    assert_eq!(sym_entry.name, "TokenService");
    assert!(code_slice.contains("pub struct TokenService"));
    assert!(!code_slice.contains("create_token"), "Extraction should not include unrelated symbols");
}
