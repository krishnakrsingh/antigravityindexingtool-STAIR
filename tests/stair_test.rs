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
fn test_ast_outline_parser_fixes() {
    // 1. TypeScript with multi-line signatures, arrow function properties, and inline parameter braces
    let ts_code = r#"
export class ProjectStore {
    @action
    setSection = (section: string) => {
        this.section = section;
    };

    @action("FETCH")
    fetchProjects = async (workspaceSlug: string) => {
        return [];
    };

    public async fetchProjectMembers(
        workspaceSlug: string,
        projectId: string,
        viewProps: { sort_order: number }
    ): Promise<{
        sort_order: number;
    }> {
        return ["admin", "member"];
    }
}

export function renderHeader(
    viewProps: { sort_order: number }
) {
    return true;
}
"#;
    let ts_symbols = AstOutlineParser::parse_file(std::path::Path::new("project.ts"), ts_code);

    // Problem B check: arrow function properties extract identifier before '='
    assert!(ts_symbols.iter().any(|s| s.name == "ProjectStore::setSection" && s.kind == SymbolKind::Method));
    assert!(ts_symbols.iter().any(|s| s.name == "ProjectStore::fetchProjects" && s.kind == SymbolKind::Method));
    assert!(!ts_symbols.iter().any(|s| s.name.contains("::=")));
    assert!(!ts_symbols.iter().any(|s| s.name.contains("::async")));
    assert!(!ts_symbols.iter().any(|s| s.name.contains("@action")));

    // Problem A & C check: multi-line signature with inline parameter type braces and multi-line return type braces
    let member_method = ts_symbols.iter().find(|s| s.name == "ProjectStore::fetchProjectMembers");
    assert!(member_method.is_some(), "Multi-line method must be parsed");
    let m = member_method.unwrap();
    assert!(m.line_end >= 20, "Must not terminate block prematurely on param or return type braces (ended at {})", m.line_end);

    let top_fn = ts_symbols.iter().find(|s| s.name == "renderHeader");
    assert!(top_fn.is_some(), "Top-level multi-line function with inline braces must be parsed");

    // 2. Python with multi-line decorators and multi-line def signatures
    let py_code = r#"
@allow_permission([ROLE.ADMIN])
class ProjectMemberViewSet:
    @allow_permission([ROLE.ADMIN])
    def partial_update(self, request, slug, project_id, pk):
        return True

    @action(detail=True, methods=["get"])
    def audit_logs(self, request, slug):
        return []

    @extend_schema(
        operation_id="get_member_detail",
        summary="Get member detail endpoint"
    )
    def detail_endpoint(self, request):
        return None

    def multi_line_def(
        self,
        request,
        slug
    ):
        result = "ok"
        return result
"#;
    let py_symbols = AstOutlineParser::parse_file(std::path::Path::new("member.py"), py_code);

    let cls = py_symbols.iter().find(|s| s.name == "ProjectMemberViewSet").expect("Class should be parsed");
    assert_eq!(cls.decorators, vec!["@allow_permission([ROLE.ADMIN])"]);

    let method = py_symbols.iter().find(|s| s.name == "ProjectMemberViewSet::partial_update").expect("partial_update should be parsed");
    assert_eq!(method.decorators, vec!["@allow_permission([ROLE.ADMIN])"]);
    assert_eq!(method.line_start, 4, "line_start should include decorator");

    let action_method = py_symbols.iter().find(|s| s.name == "ProjectMemberViewSet::audit_logs").expect("audit_logs should be parsed");
    assert_eq!(action_method.decorators, vec!["@action(detail=True, methods=[\"get\"])"]);

    let detail_method = py_symbols.iter().find(|s| s.name == "ProjectMemberViewSet::detail_endpoint").expect("detail_endpoint should be parsed");
    assert!(!detail_method.decorators.is_empty(), "Multi-line decorator must be captured");
    assert!(detail_method.decorators[0].contains("@extend_schema"));

    let multi_line_fn = py_symbols.iter().find(|s| s.name == "ProjectMemberViewSet::multi_line_def").expect("multi_line_def should be parsed");
    assert_eq!(multi_line_fn.line_start, 19, "multi_line_def line_start");
    assert_eq!(multi_line_fn.line_end, 25, "multi_line_def must encompass method body past ): to line 25");
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

#[test]
fn test_symbol_trie_crlf_offset_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    // 1. Rust file with Windows CRLF line endings and padding lines
    let mut rust_content = String::new();
    for i in 0..60 {
        rust_content.push_str(&format!("// Padding line {:02}\r\n", i));
    }
    rust_content.push_str("pub struct WindowsService {\r\n");
    rust_content.push_str("    pub port: u16,\r\n");
    rust_content.push_str("}\r\n\r\n");
    rust_content.push_str("impl WindowsService {\r\n");
    rust_content.push_str("    pub fn start(&self) -> bool {\r\n");
    rust_content.push_str("        self.port > 0\r\n");
    rust_content.push_str("    }\r\n");
    rust_content.push_str("}\r\n\r\n");
    rust_content.push_str("pub fn run_service() -> bool {\r\n");
    rust_content.push_str("    true\r\n");
    rust_content.push_str("}\r\n");

    let rs_path = src.join("win_service.rs");
    fs::write(&rs_path, &rust_content).unwrap();

    // 2. Python file with Windows CRLF line endings (simulating Plane models)
    let mut py_content = String::new();
    for i in 0..80 {
        py_content.push_str(&format!("# Python padding line {:02}\r\n", i));
    }
    py_content.push_str("class ProjectMember:\r\n");
    py_content.push_str("    role: int\r\n\r\n");
    py_content.push_str("    def __str__(self):\r\n");
    py_content.push_str("        return f\"{self.project.name} {self.member.name}\"\r\n\r\n");
    py_content.push_str("    def is_admin(self) -> bool:\r\n");
    py_content.push_str("        return self.role == 20\r\n");

    let py_path = src.join("project.py");
    fs::write(&py_path, &py_content).unwrap();

    let trie = SymbolTrie::new(root);
    let count = trie.build();
    assert!(count >= 5);

    // Verify Rust method start begins and ends exactly at method definition
    let (sym, code) = trie.extract_symbol_code("src/win_service.rs::WindowsService::start").expect("Method start should extract");
    assert_eq!(sym.name, "WindowsService::start");
    assert!(code.starts_with("    pub fn start(&self) -> bool {"));
    assert!(code.contains("self.port > 0"));
    assert!(code.contains("    }"));
    assert!(!code.contains("WindowsService {"));
    assert!(!code.contains("run_service"));

    // Verify Rust standalone function
    let (fn_sym, fn_code) = trie.extract_symbol_code("src/win_service.rs::run_service").expect("Function should extract");
    assert_eq!(fn_sym.name, "run_service");
    assert!(fn_code.starts_with("pub fn run_service() -> bool {"));
    assert!(fn_code.contains("true"));

    // Verify Python method definition with CRLF
    let (py_sym, py_code) = trie.extract_symbol_code("src/project.py::ProjectMember::__str__").expect("Python __str__ should extract");
    assert_eq!(py_sym.name, "ProjectMember::__str__");
    assert!(py_code.starts_with("    def __str__(self):"));
    assert!(py_code.contains("return f\"{self.project.name} {self.member.name}\""));
    assert!(!py_code.contains("Python padding line"));
    assert!(!py_code.contains("is_admin"));
}
