pub mod handlers;
pub mod pool;
pub mod protocol;

pub use handlers::McpHandler;
#[allow(unused_imports)]
pub use pool::{WorkspacePool, WorkspaceState};
pub use protocol::*;

use serde_json::json;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

pub fn run_stdio_server(workspace_root: PathBuf) -> io::Result<()> {
    let handler = McpHandler::new(workspace_root);
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line_res in stdin.lock().lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let resp_str = serde_json::to_string(&err_resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
                continue;
            }
        };

        // Handle notifications (no response needed)
        if request.id.is_none() {
            continue;
        }

        let req_id = request.id.clone();

        match request.method.as_str() {
            "initialize" => {
                // Dynamically update default workspace from initialize params if provided
                if let Some(params) = &request.params {
                    if let Some(root_uri) = params.get("rootUri").and_then(|v| v.as_str()) {
                        let p = pool::uri_to_path(root_uri);
                        handler.pool.set_default(p);
                    } else if let Some(folders) = params.get("workspaceFolders").and_then(|v| v.as_array()) {
                        if let Some(first) = folders.first() {
                            if let Some(uri) = first.get("uri").and_then(|v| v.as_str()) {
                                let p = pool::uri_to_path(uri);
                                handler.pool.set_default(p);
                            }
                        }
                    }
                }

                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "antigravity-fast-core",
                            "version": "0.1.0"
                        }
                    })),
                    error: None,
                };
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
            }
            "ping" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(json!({})),
                    error: None,
                };
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
            }
            "tools/list" => {
                let tools = McpHandler::list_tools();
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(json!({
                        "tools": tools
                    })),
                    error: None,
                };
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
            }
            "tools/call" => {
                let params = request.params.unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let default_args = json!({});
                let tool_args = params.get("arguments").unwrap_or(&default_args);

                let tool_result = handler.handle_tool_call(tool_name, tool_args);
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(serde_json::to_value(tool_result).unwrap_or_default()),
                    error: None,
                };
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
            }
            _ => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", request.method),
                        data: None,
                    }),
                };
                let resp_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(stdout, "{}", resp_str);
                let _ = stdout.flush();
            }
        }
    }

    Ok(())
}
