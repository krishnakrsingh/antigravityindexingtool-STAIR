use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpTextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpTextContent>,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

/// Robust deserializer handling arrays of strings, JSON strings, single strings, or null
pub fn deserialize_decorators<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DecoratorFormat {
        List(Vec<String>),
        Str(String),
        None,
    }

    match DecoratorFormat::deserialize(deserializer)? {
        DecoratorFormat::List(vec) => Ok(vec),
        DecoratorFormat::Str(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(Vec::new())
            } else if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
                Ok(parsed)
            } else {
                Ok(vec![trimmed.to_string()])
            }
        }
        DecoratorFormat::None => Ok(Vec::new()),
    }
}

/// Enriched AST node metadata for Code RAG and semantic search results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichedAstNode {
    /// Full hierarchical symbol path, e.g. "apps/api/plane/app/views/project/member.py::ProjectMemberViewSet::audit_logs"
    #[serde(default, alias = "node_id", alias = "id")]
    pub symbol_path: String,

    /// Relative file path, e.g. "apps/api/plane/app/views/project/member.py"
    #[serde(default, alias = "file")]
    pub file_path: String,

    /// Symbol identifier, e.g. "audit_logs"
    #[serde(default, alias = "symbol_name")]
    pub name: String,

    /// Symbol kind: "class", "method", "function", "struct", "enum", "interface"
    #[serde(default, alias = "symbol_type")]
    pub kind: String,

    /// Enclosing class name if method/property, or None if top-level
    #[serde(default, alias = "class", skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,

    /// Complete function/method/class signature
    #[serde(default)]
    pub signature: String,

    /// List of decorators applied to this node (e.g. "@action(...)")
    #[serde(default, deserialize_with = "deserialize_decorators")]
    pub decorators: Vec<String>,

    /// Extracted docstring / documentation comment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,

    /// 1-based start line
    #[serde(default)]
    pub line_start: usize,

    /// 1-based end line
    #[serde(default)]
    pub line_end: usize,

    /// 0-based start byte
    #[serde(default)]
    pub byte_start: usize,

    /// 0-based end byte
    #[serde(default)]
    pub byte_end: usize,

    /// Relevance / cosine similarity score (0.0 to 1.0)
    #[serde(default)]
    pub score: f32,

    /// Code snippet preview (first 15-20 lines or complete definition)
    #[serde(default)]
    pub preview: String,
}

/// Structured response container for Vector RAG semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRagResponse {
    pub query: String,
    pub backend: String,
    pub total: usize,
    pub results: Vec<EnrichedAstNode>,
}
