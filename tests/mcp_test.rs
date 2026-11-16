use antigravity_fast_core::McpHandler;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_mcp_handler_tool_calls() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    let main_rs = src.join("main.rs");
    fs::write(
        &main_rs,
        "pub fn run_fast_intelligence() {\n    println!(\"instant speed\");\n}\n",
    )
    .unwrap();

    let handler = McpHandler::new(root);

    // 1. Tool listing test
    let tools = McpHandler::list_tools();
    assert!(tools.len() >= 6);
    assert!(tools.iter().any(|t| t.name == "instant_grep"));
    assert!(tools.iter().any(|t| t.name == "get_symbol_outline"));
    assert!(tools.iter().any(|t| t.name == "get_symbol_definition"));
    assert!(tools.iter().any(|t| t.name == "merkle_diff"));

    // 2. instant_grep tool call
    let grep_res = handler.handle_tool_call(
        "instant_grep",
        &json!({
            "pattern": "run_fast_intelligence"
        }),
    );
    assert!(!grep_res.is_error);
    assert!(grep_res.content[0].text.contains("run_fast_intelligence"));

    // 3. get_symbol_outline tool call
    let outline_res = handler.handle_tool_call("get_symbol_outline", &json!({}));
    assert!(!outline_res.is_error);
    assert!(outline_res.content[0].text.contains("run_fast_intelligence"));

    // 4. merkle_snapshot tool call
    let snap_res = handler.handle_tool_call("merkle_snapshot", &json!({}));
    assert!(!snap_res.is_error);
    assert!(snap_res.content[0].text.contains("BLAKE3 Merkle Root"));
}
