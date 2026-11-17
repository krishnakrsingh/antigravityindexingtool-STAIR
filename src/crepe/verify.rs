use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn find_best_rg_path() -> PathBuf {
    if let Ok(p) = std::env::var("RG_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    // Check local Antigravity IDE bundled ripgrep
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(&local_app)
            .join("Programs")
            .join("Antigravity IDE")
            .join("resources")
            .join("app")
            .join("node_modules")
            .join("@vscode")
            .join("ripgrep")
            .join("bin")
            .join(if cfg!(windows) { "rg.exe" } else { "rg" });
        if p.exists() {
            return p;
        }

        // Check local VS Code bundled ripgrep as secondary fallback
        let vscode_p = PathBuf::from(&local_app)
            .join("Programs")
            .join("Microsoft VS Code")
            .join("resources")
            .join("app")
            .join("node_modules")
            .join("@vscode")
            .join("ripgrep")
            .join("bin")
            .join(if cfg!(windows) { "rg.exe" } else { "rg" });
        if vscode_p.exists() {
            return vscode_p;
        }
    }
    // Check system PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let exe = if cfg!(windows) { dir.join("rg.exe") } else { dir.join("rg") };
            if exe.exists() {
                return exe;
            }
        }
    }
    // Default system executable name
    PathBuf::from(if cfg!(windows) { "rg.exe" } else { "rg" })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line_number: usize,
    pub column: usize,
    pub line_content: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub pattern: String,
    pub candidates_count: usize,
    pub matches_count: usize,
    pub duration_ms: f64,
    pub engine_used: String,
    pub matches: Vec<SearchMatch>,
}

pub struct Verifier {
    workspace_root: PathBuf,
    rg_path: PathBuf,
}

impl Verifier {
    pub fn new(workspace_root: PathBuf) -> Self {
        let rg_path = find_best_rg_path();
        Self {
            workspace_root,
            rg_path,
        }
    }

    /// Direct whole-workspace ripgrep search for cold starts and background warmup
    pub fn search_workspace_ripgrep(
        &self,
        pattern: &str,
        path_filter: Option<&str>,
        case_sensitive: bool,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>, String> {
        if !self.rg_path.exists() {
            return Err("Ripgrep binary not found".to_string());
        }

        let mut cmd = Command::new(&self.rg_path);
        cmd.arg("--vimgrep")
            .arg("--no-heading")
            .arg("--color")
            .arg("never")
            .arg("--max-count")
            .arg(max_results.to_string());

        if !case_sensitive {
            cmd.arg("-i");
        } else {
            cmd.arg("-s");
        }

        if let Some(pf) = path_filter {
            cmd.arg("-g").arg(format!("*{}*", pf));
        }

        cmd.arg("-g").arg("!.git")
            .arg("-g").arg("!node_modules")
            .arg("-g").arg("!target")
            .arg("-e").arg(pattern)
            .arg(&self.workspace_root);

        let output = cmd.output().map_err(|e| format!("Failed to spawn rg: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut matches = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(4, ':').collect();
            if parts.len() >= 4 {
                let file_path = parts[0];
                let line_number = parts[1].parse::<usize>().unwrap_or(0);
                let column = parts[2].parse::<usize>().unwrap_or(0);
                let content = parts[3].to_string();

                let rel_file = std::path::Path::new(file_path)
                    .strip_prefix(&self.workspace_root)
                    .unwrap_or_else(|_| std::path::Path::new(file_path))
                    .to_string_lossy()
                    .replace('\\', "/");

                matches.push(SearchMatch {
                    file: rel_file,
                    line_number,
                    column,
                    line_content: content,
                });

                if matches.len() >= max_results {
                    break;
                }
            }
        }

        Ok(matches)
    }

    /// Verifies candidate files using the bundled Ripgrep binary
    pub fn verify_with_ripgrep(
        &self,
        pattern: &str,
        candidates: &[String],
        case_sensitive: bool,
    ) -> Result<Vec<SearchMatch>, String> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        if !self.rg_path.exists() {
            return Err(format!("Bundled ripgrep not found at {:?}", self.rg_path));
        }

        let mut matches = Vec::new();
        // Chunk candidates to avoid Windows 8191 character command line overflow
        let chunk_size = 25;

        for chunk in candidates.chunks(chunk_size) {
            let mut cmd = Command::new(&self.rg_path);
            cmd.current_dir(&self.workspace_root)
                .arg("--line-number")
                .arg("--column")
                .arg("--no-heading")
                .arg("--color")
                .arg("never");

            if !case_sensitive {
                cmd.arg("-i");
            }

            cmd.arg("-e").arg(pattern);

            for cand in chunk {
                cmd.arg(cand);
            }

            let output = cmd.output().map_err(|e| e.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout);

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 4 {
                    let (file, line_str, col_str, content) = if parts[0].len() == 1
                        && parts.get(1).map_or(false, |p| p.starts_with('\\') || p.starts_with('/'))
                    {
                        // Windows drive prefix e.g. C:\path\to\file.rs:10:5:code
                        let full_file = format!("{}:{}", parts[0], parts[1]);
                        let l = parts.get(2).copied().unwrap_or("0");
                        let c = parts.get(3).copied().unwrap_or("0");
                        let rest = parts[4..].join(":");
                        (full_file, l, c, rest)
                    } else {
                        let full_file = parts[0].to_string();
                        let l = parts.get(1).copied().unwrap_or("0");
                        let c = parts.get(2).copied().unwrap_or("0");
                        let rest = parts[3..].join(":");
                        (full_file, l, c, rest)
                    };

                    let rel_file = if let Ok(rel) = PathBuf::from(&file).strip_prefix(&self.workspace_root) {
                        rel.to_string_lossy().replace('\\', "/")
                    } else {
                        file.replace('\\', "/")
                    };

                    let line_number = line_str.parse::<usize>().unwrap_or(0);
                    let column = col_str.parse::<usize>().unwrap_or(0);

                    matches.push(SearchMatch {
                        file: rel_file,
                        line_number,
                        column,
                        line_content: content,
                    });
                }
            }
        }

        Ok(matches)
    }

    /// In-process native Rust regex verification (zero process spawn overhead, sub-millisecond)
    pub fn verify_in_process(
        &self,
        pattern: &str,
        candidates: &[String],
        case_sensitive: bool,
    ) -> Result<Vec<SearchMatch>, String> {
        let re = RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|e| format!("Invalid regex pattern: {}", e))?;

        let mut matches = Vec::new();

        for cand in candidates {
            let abs_path = self.workspace_root.join(cand);
            if let Ok(content) = fs::read_to_string(&abs_path) {
                for (line_idx, line) in content.lines().enumerate() {
                    if let Some(mat) = re.find(line) {
                        matches.push(SearchMatch {
                            file: cand.replace('\\', "/"),
                            line_number: line_idx + 1,
                            column: mat.start() + 1,
                            line_content: line.to_string(),
                        });
                    }
                }
            }
        }

        Ok(matches)
    }
}
