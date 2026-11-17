use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCacheMeta {
    pub workspace_path: String,
    pub workspace_name: String,
    pub created_at: String,
    pub last_accessed: String,
    #[serde(default)]
    pub total_files: usize,
    #[serde(default)]
    pub total_bytes: u64,
}

pub fn get_base_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FAST_CORE_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("antigravity-fast-core").join("cache")
    } else if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        PathBuf::from(home).join(".cache").join("antigravity-fast-core")
    } else {
        std::env::temp_dir().join("antigravity-fast-core").join("cache")
    }
}

pub fn get_workspace_cache_dir(workspace_root: &Path) -> PathBuf {
    let canonical = fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    let path_str = canonical.to_string_lossy().to_lowercase().replace('\\', "/");
    let hash = blake3::hash(path_str.as_bytes()).to_hex().to_string();
    let short_hash = &hash[..16];
    get_base_cache_dir().join(short_hash)
}

pub fn save_workspace_meta(workspace_root: &Path, cache_dir: &Path, total_files: usize) {
    let canonical = fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    let name = canonical
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string());

    let now = chrono::Utc::now().to_rfc3339();
    let meta = WorkspaceCacheMeta {
        workspace_path: canonical.to_string_lossy().to_string(),
        workspace_name: name,
        created_at: now.clone(),
        last_accessed: now,
        total_files,
        total_bytes: calculate_dir_size(cache_dir),
    };

    let _ = fs::create_dir_all(cache_dir);
    let meta_path = cache_dir.join("workspace_meta.json");
    if let Ok(json_str) = serde_json::to_string_pretty(&meta) {
        let _ = fs::write(meta_path, json_str);
    }
}

pub fn touch_workspace_meta(cache_dir: &Path) {
    let meta_path = cache_dir.join("workspace_meta.json");
    if let Ok(content) = fs::read_to_string(&meta_path) {
        if let Ok(mut meta) = serde_json::from_str::<WorkspaceCacheMeta>(&content) {
            meta.last_accessed = chrono::Utc::now().to_rfc3339();
            if let Ok(json_str) = serde_json::to_string_pretty(&meta) {
                let _ = fs::write(meta_path, json_str);
            }
        }
    }
}

pub fn calculate_dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = walkdir::WalkDir::new(dir).into_iter().collect::<Result<Vec<_>, _>>() {
        for entry in entries {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

pub fn list_caches() -> Vec<(PathBuf, WorkspaceCacheMeta)> {
    let base = get_base_cache_dir();
    let mut results = Vec::new();

    if let Ok(entries) = fs::read_dir(&base) {
        for entry_res in entries.flatten() {
            let path = entry_res.path();
            if path.is_dir() {
                let meta_path = path.join("workspace_meta.json");
                if let Ok(content) = fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<WorkspaceCacheMeta>(&content) {
                        results.push((path, meta));
                    }
                }
            }
        }
    }
    results
}

pub fn clean_cache(workspace_root: &Path) -> io::Result<bool> {
    let dir = get_workspace_cache_dir(workspace_root);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn clean_all_caches() -> io::Result<usize> {
    let base = get_base_cache_dir();
    let mut count = 0;
    if base.exists() {
        if let Ok(entries) = fs::read_dir(&base) {
            for entry_res in entries.flatten() {
                let path = entry_res.path();
                if path.is_dir() {
                    if fs::remove_dir_all(&path).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    Ok(count)
}
