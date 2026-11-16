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

impl UserSession {
    pub fn is_valid(&self) -> bool {
        self.id > 0
    }
}

pub fn validate_session(session: &UserSession) -> bool {
    session.is_valid()
}
"#;

    let symbols = AstOutlineParser::parse_file(std::path::Path::new("session.rs"), code);
    // Should include: UserSession struct, SessionState enum, UserSession::is_valid method, UserSession impl, validate_session fn
    assert!(symbols.iter().any(|s| s.name == "UserSession" && s.kind == SymbolKind::Struct));
    assert!(symbols.iter().any(|s| s.name == "SessionState" && s.kind == SymbolKind::Enum));
    assert!(symbols.iter().any(|s| s.name == "UserSession::is_valid" && s.kind == SymbolKind::Method));
    assert!(symbols.iter().any(|s| s.name == "validate_session" && s.kind == SymbolKind::Function));
}

#[test]
fn test_ast_outline_parsing_typescript_and_python() {
    // TypeScript test
    let ts_code = r#"
export class AuthService {
    private secret: string;

    async authenticate(token: string): Promise<boolean> {
        return token.length > 10;
    }
}
"#;
    let ts_symbols = AstOutlineParser::parse_file(std::path::Path::new("auth.ts"), ts_code);
    assert!(ts_symbols.iter().any(|s| s.name == "AuthService" && s.kind == SymbolKind::Class));
    assert!(ts_symbols.iter().any(|s| s.name == "AuthService::authenticate" && s.kind == SymbolKind::Method));

    // Python test
    let py_code = r#"
class PaymentProcessor:
    def __init__(self, key: str):
        self.key = key

    def process_charge(self, amount: int) -> bool:
        return amount > 0
"#;
    let py_symbols = AstOutlineParser::parse_file(std::path::Path::new("pay.py"), py_code);
    assert!(py_symbols.iter().any(|s| s.name == "PaymentProcessor" && s.kind == SymbolKind::Class));
    assert!(py_symbols.iter().any(|s| s.name == "PaymentProcessor::process_charge" && s.kind == SymbolKind::Method));
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

impl TokenService {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn verify(&self, token: &str) -> bool {
        !token.is_empty()
    }
}

pub fn create_token() -> String {
    "xyz123".to_string()
}
"#;
    fs::write(&file_path, content).unwrap();

    let trie = SymbolTrie::new(root);
    let count = trie.build();
    assert!(count >= 4);

    // Exact lookup test for struct
    let struct_path = "src/service.rs::TokenService";
    let entry = trie.find_exact(struct_path);
    assert!(entry.is_some(), "Exact struct path must resolve");
    assert_eq!(entry.unwrap().name, "TokenService");

    // Exact lookup test for method inside impl block!
    let method_path = "src/service.rs::TokenService::verify";
    let m_entry = trie.find_exact(method_path);
    assert!(m_entry.is_some(), "Method path inside impl must resolve");
    assert_eq!(m_entry.unwrap().kind, SymbolKind::Method);

    // Non-existent symbol must return None (0.00% hallucination)
    let fake_path = "src/service.rs::NonExistentSymbol";
    assert!(trie.find_exact(fake_path).is_none(), "Must guarantee zero hallucination");

    // Extraction test: method code extraction
    let (sym_entry, code_slice) = trie.extract_symbol_code(method_path).expect("Method extraction failed");
    assert_eq!(sym_entry.name, "TokenService::verify");
    assert!(code_slice.contains("pub fn verify"));
    assert!(code_slice.contains("!token.is_empty()"));
    assert!(!code_slice.contains("create_token"), "Extraction should not include unrelated symbols");
}
