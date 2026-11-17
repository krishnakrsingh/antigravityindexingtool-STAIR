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
                // If running/locked on Windows (os error 5: Access is denied), rename active binary to .old
                let old_exe = bin_dir.join(format!("{}.old", exe_name));
                let _ = fs::remove_file(&old_exe);
                if fs::rename(&target_exe, &old_exe).is_ok() && fs::copy(&current_exe, &target_exe).is_ok() {
                    println!("  ✅ Overwrote active binary (previous version archived to .old).");
                } else {
                    println!("  ⚠️ Note: Could not overwrite binary directly (might be running): {}", e);
                    println!("  Using current binary location: {:?}", current_exe);
                }
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
    let mut fast_core_server = json!({
        "command": effective_exe.to_string_lossy().to_string(),
        "args": ["--mcp"]
    });
    if rg_path.exists() {
        fast_core_server["env"] = json!({
            "RG_PATH": rg_path.to_string_lossy().to_string()
        });
    }

    let plugin_config = json!({
        "name": "fast-code-intelligence",
        "version": "1.0.0",
        "description": "High-speed native code intelligence layer powered by Rust: BLAKE3 Merkle tree, Cursor-style sparse n-gram inverted index (crepe), Code STAIR AST symbol extraction, and GPU-accelerated code embeddings.",
        "author": "Antigravity Engineering",
        "mcpServers": {
            "fast-core": fast_core_server
        }
    });

    fs::write(
        &plugin_json_path,
        serde_json::to_string_pretty(&plugin_config)?,
    )?;
    println!("  ✅ Plugin manifest written to: {:?}", plugin_json_path);

    // Also sync global ~/.gemini/config/mcp_config.json for Antigravity IDE
    let mcp_config_path = gemini_dir.join("config").join("mcp_config.json");
    let mut mcp_obj = if mcp_config_path.exists() {
        fs::read_to_string(&mcp_config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_else(|| json!({"mcpServers": {}}))
    } else {
        json!({"mcpServers": {}})
    };
    if let Some(servers) = mcp_obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.insert("fast-core".to_string(), fast_core_server.clone());
        let _ = fs::write(&mcp_config_path, serde_json::to_string_pretty(&mcp_obj)?);
        println!("  ✅ Synced global Antigravity config: {:?}", mcp_config_path);
    }

    // Write SKILL.md into ~/.gemini/config/plugins/fast-code-intelligence/skills/fast-code-intelligence/
    let skill_dir = plugin_dir.join("skills").join("fast-code-intelligence");
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    let skill_content = include_str!("../skills/fast-code-intelligence/SKILL.md");
    fs::write(&skill_path, skill_content)?;
    println!("  ✅ Synced Antigravity Skill: {:?}", skill_path);

    // Write execution-guardrails.md and no-file-paging.md into plugin rules & global config rules
    let guardrails_content = include_str!("../rules/execution-guardrails.md");
    let no_paging_content = include_str!("../rules/no-file-paging.md");
    let plugin_rules_dir = plugin_dir.join("rules");
    fs::create_dir_all(&plugin_rules_dir)?;
    let plugin_guardrail_path = plugin_rules_dir.join("execution-guardrails.md");
    fs::write(&plugin_guardrail_path, guardrails_content)?;
    let plugin_paging_path = plugin_rules_dir.join("no-file-paging.md");
    fs::write(&plugin_paging_path, no_paging_content)?;
    println!("  ✅ Synced Plugin Execution Guardrail: {:?}", plugin_guardrail_path);

    let global_rules_dir = gemini_dir.join("config").join("rules");
    fs::create_dir_all(&global_rules_dir)?;
    let global_guardrail_path = global_rules_dir.join("execution-guardrails.md");
    fs::write(&global_guardrail_path, guardrails_content)?;
    let global_paging_path = global_rules_dir.join("no-file-paging.md");
    fs::write(&global_paging_path, no_paging_content)?;
    println!("  ✅ Synced Global Agent Execution Guardrail: {:?}", global_guardrail_path);

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
