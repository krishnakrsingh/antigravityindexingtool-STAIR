use ignore::WalkBuilder;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Default)]
struct TreeNode {
    is_dir: bool,
    children: BTreeMap<String, TreeNode>,
}

pub struct CodebaseTree {
    root: PathBuf,
    max_depth: usize,
}

impl CodebaseTree {
    pub fn new(root: PathBuf, max_depth: usize) -> Self {
        Self { root, max_depth }
    }

    pub fn generate(&self) -> (String, usize) {
        let mut walker = WalkBuilder::new(&self.root);
        walker
            .hidden(true)
            .git_ignore(true)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".git"
                    && name != "target"
                    && name != "node_modules"
                    && name != ".antigravity"
                    && name != ".next"
                    && name != "dist"
                    && name != "build"
                    && name != "__pycache__"
                    && name != ".turbo"
            });

        let mut root_node = TreeNode {
            is_dir: true,
            children: BTreeMap::new(),
        };

        let mut total_files = 0;

        for result in walker.build() {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path == self.root {
                continue;
            }

            let rel_path = match path.strip_prefix(&self.root) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let is_dir = path.is_dir();
            if !is_dir {
                total_files += 1;
            }

            let parts: Vec<String> = rel_path
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();

            if parts.len() > self.max_depth + 1 {
                continue;
            }

            let mut curr = &mut root_node;
            for (idx, part) in parts.iter().enumerate() {
                let at_end = idx == parts.len() - 1;
                curr = curr.children.entry(part.clone()).or_insert_with(|| TreeNode {
                    is_dir: if at_end { is_dir } else { true },
                    children: BTreeMap::new(),
                });
            }
        }

        let mut out = String::new();
        let ws_name = self
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string());

        out.push_str(&format!(
            "📁 Codebase Structure: {} ({} files indexed, gitignore respected)\n",
            ws_name, total_files
        ));
        Self::render_node(&root_node, "", &mut out, 0, self.max_depth);

        (out, total_files)
    }

    fn render_node(
        node: &TreeNode,
        prefix: &str,
        out: &mut String,
        current_depth: usize,
        max_depth: usize,
    ) {
        if current_depth > max_depth {
            return;
        }

        let total_children = node.children.len();
        let max_display_children = 40;

        for (idx, (name, child)) in node.children.iter().enumerate() {
            if idx >= max_display_children {
                let remaining = total_children - max_display_children;
                out.push_str(&format!("{}└── ... ({} more items)\n", prefix, remaining));
                break;
            }

            let last_child = idx == total_children - 1 || idx == max_display_children - 1;
            let connector = if last_child { "└── " } else { "├── " };
            let suffix = if child.is_dir { "/" } else { "" };

            out.push_str(&format!("{}{}{}{}\n", prefix, connector, name, suffix));

            if child.is_dir && !child.children.is_empty() && current_depth < max_depth {
                let new_prefix = format!("{}{}", prefix, if last_child { "    " } else { "│   " });
                Self::render_node(
                    child,
                    &new_prefix,
                    out,
                    current_depth + 1,
                    max_depth,
                );
            }
        }
    }

    pub fn handle_session_start_hook(default_ws: PathBuf) -> io::Result<()> {
        let mut input_str = String::new();
        let _ = io::stdin().read_to_string(&mut input_str);

        let ws_path = if !input_str.trim().is_empty() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&input_str) {
                if let Some(cwd) = val.get("cwd").and_then(|v| v.as_str()) {
                    PathBuf::from(cwd)
                } else if let Some(ws) = val
                    .get("workspacePaths")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                {
                    PathBuf::from(ws)
                } else {
                    default_ws
                }
            } else {
                default_ws
            }
        } else {
            default_ws
        };

        let tree = CodebaseTree::new(ws_path, 4);
        let (tree_text, _file_count) = tree.generate();

        let response = json!({
            "injectSteps": [
                {
                    "systemMessage": {
                        "systemMessage": format!(
                            "⚡ [Resident Codebase Map]\nBelow is the verified structural file tree of this workspace (ignoring target/node_modules/build artifacts). Use this map to navigate directly to the right file without running directory crawl or search commands.\n\n```\n{}\n```",
                            tree_text
                        )
                    }
                }
            ]
        });

        println!("{}", serde_json::to_string(&response)?);
        Ok(())
    }
}
