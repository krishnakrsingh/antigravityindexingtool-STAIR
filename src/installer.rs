use crate::crepe::verify::find_best_rg_path;
use crate::mcp::handlers::McpHandler;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub fn get_global_bin_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".antigravity").join("bin")
}

pub fn run_setup() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!(" Antigravity Fast-Core: Production Installer & Auto-Setup   ");
    println!("============================================================");

    let current_exe = std::env::current_exe()?;
    let bin_dir = get_global_bin_dir();
    fs::create_dir_all(&bin_dir)?;

    let exe_name = if cfg!(windows) {
        "antigravity-fast-core.exe"
    } else {
        "antigravity-fast-core"
    };
    let target_exe = bin_dir.join(exe_name);

    // 1. Copy binary to permanent global location
    if current_exe != target_exe {
        println!("\n[1/4] Installing binary to {:?}...", target_exe);
        match fs::copy(&current_exe, &target_exe) {
            Ok(_) => println!("  ✅ Installed binary successfully."),
            Err(e) => {
                println!("  ⚠️ Note: Could not overwrite binary directly (might be running): {}", e);
                println!("  Using current binary location: {:?}", current_exe);
            }
        }
    } else {
        println!("\n[1/4] Binary is already running from global location: {:?}", target_exe);
    }

    let effective_exe = if target_exe.exists() {
        target_exe.clone()
    } else {
        current_exe.clone()
    };

    // 2. Detect Ripgrep path
    let rg_path = find_best_rg_path();
    println!("\n[2/4] Detecting ripgrep engine...");
    println!("  ✅ Ripgrep binary located at: {:?}", rg_path);

    // 3. Configure Antigravity Plugin & MCP Schemas
    println!("\n[3/4] Configuring Antigravity MCP Server & Tool Schemas...");
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let gemini_dir = PathBuf::from(&home).join(".gemini");
    let plugin_dir = gemini_dir.join("config").join("plugins").join("fast-code-intelligence");
    let mcp_schemas_dir = gemini_dir.join("antigravity").join("mcp").join("fast-core");

    fs::create_dir_all(&plugin_dir)?;
    fs::create_dir_all(&mcp_schemas_dir)?;

    let plugin_json_path = plugin_dir.join("plugin.json");
    let plugin_config = json!({
        "name": "fast-code-intelligence",
        "version": "1.0.0",
        "description": "High-speed native code intelligence layer powered by Rust: BLAKE3 Merkle tree, Cursor-style sparse n-gram inverted index (crepe), Code STAIR AST symbol extraction, and GPU-accelerated code embeddings.",
        "author": "Antigravity Engineering",
        "mcpServers": {
            "fast-core": {
                "command": effective_exe.to_string_lossy().to_string(),
                "args": ["--mcp"],
                "env": {
                    "RG_PATH": rg_path.to_string_lossy().to_string()
                }
            }
        }
    });

    fs::write(
        &plugin_json_path,
        serde_json::to_string_pretty(&plugin_config)?,
    )?;
    println!("  ✅ Plugin manifest written to: {:?}", plugin_json_path);

    // Write all tool schemas into ~/.gemini/antigravity/mcp/fast-core/
    let tools = McpHandler::list_tools();
    for tool in tools {
        let schema_path = mcp_schemas_dir.join(format!("{}.json", tool.name));
        let schema_val = json!({
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema
        });
        fs::write(
            &schema_path,
            serde_json::to_string(&schema_val)?,
        )?;
        println!("  ✅ Synced MCP tool schema: {}.json", tool.name);
    }

    // 4. Third-Party Integration: Claude Desktop
    println!("\n[4/4] Checking third-party IDE configurations...");
    if let Ok(app_data) = std::env::var("APPDATA") {
        let claude_config_path = PathBuf::from(app_data)
            .join("Claude")
            .join("claude_desktop_config.json");
        if claude_config_path.exists() {
            if let Ok(content) = fs::read_to_string(&claude_config_path) {
                if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(servers) = val.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                        servers.insert(
                            "fast-core".to_string(),
                            json!({
                                "command": effective_exe.to_string_lossy().to_string(),
                                "args": ["--mcp"]
                            }),
                        );
                        let _ = fs::write(
                            &claude_config_path,
                            serde_json::to_string_pretty(&val)?,
                        );
                        println!("  ✅ Auto-registered with Claude Desktop: {:?}", claude_config_path);
                    }
                }
            }
        }
    }

    println!("\n============================================================");
    println!(" ✅ Antigravity Fast-Core successfully configured for PROD! ");
    println!(" Binary: {:?}", effective_exe);
    println!(" Schemas: {:?}", mcp_schemas_dir);
    println!("============================================================");

    Ok(())
}
