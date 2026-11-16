use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_BUNDLED_RG: &str =
    r#"C:\Users\krish\AppData\Local\Programs\Antigravity IDE\resources\app\node_modules\@vscode\ripgrep\bin\rg.exe"#;

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
        let rg_path = std::env::var("RG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_BUNDLED_RG));
        Self {
            workspace_root,
            rg_path,
        }
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
