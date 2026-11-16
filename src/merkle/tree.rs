use blake3::Hasher;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    pub name: String,
    pub rel_path: String,
    pub hash: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_secs: u64,
    pub children: BTreeMap<String, MerkleNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MerkleDiff {
    pub from_hash: String,
    pub to_hash: String,
    pub added_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub total_changes: usize,
    pub duration_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTreeSnapshot {
    pub root_hash: String,
    pub timestamp_secs: u64,
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_bytes: u64,
    pub root: MerkleNode,
}

pub struct MerkleTree {
    root_path: PathBuf,
    current_snapshot: RwLock<MerkleTreeSnapshot>,
    history: RwLock<HashMap<String, MerkleTreeSnapshot>>,
}

impl MerkleTree {
    pub fn new(root_path: PathBuf) -> Self {
        let (root_node, files, dirs, bytes) = Self::build_tree_recursive(&root_path);
        let snapshot = MerkleTreeSnapshot {
            root_hash: root_node.hash.clone(),
            timestamp_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_files: files,
            total_dirs: dirs,
            total_bytes: bytes,
            root: root_node,
        };

        let mut history = HashMap::new();
        history.insert(snapshot.root_hash.clone(), snapshot.clone());

        Self {
            root_path,
            current_snapshot: RwLock::new(snapshot),
            history: RwLock::new(history),
        }
    }

    pub fn current_root_hash(&self) -> String {
        self.current_snapshot.read().unwrap().root_hash.clone()
    }

    pub fn get_snapshot(&self) -> MerkleTreeSnapshot {
        self.current_snapshot.read().unwrap().clone()
    }

    pub fn refresh(&self) -> MerkleDiff {
        let t0 = std::time::Instant::now();
        let old_snapshot = self.current_snapshot.read().unwrap().clone();
        let (new_root, files, dirs, bytes) = Self::build_tree_recursive(&self.root_path);

        let new_snapshot = MerkleTreeSnapshot {
            root_hash: new_root.hash.clone(),
            timestamp_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_files: files,
            total_dirs: dirs,
            total_bytes: bytes,
            root: new_root,
        };

        let mut diff = Self::diff_nodes(&old_snapshot.root, &new_snapshot.root);
        diff.from_hash = old_snapshot.root_hash;
        diff.to_hash = new_snapshot.root_hash.clone();
        diff.total_changes = diff.added_files.len() + diff.modified_files.len() + diff.deleted_files.len();
        diff.duration_micros = t0.elapsed().as_micros() as u64;

        {
            let mut hist = self.history.write().unwrap();
            hist.insert(new_snapshot.root_hash.clone(), new_snapshot.clone());
        }

        *self.current_snapshot.write().unwrap() = new_snapshot;
        diff
    }

    pub fn diff_against_hash(&self, from_hash: &str) -> Option<MerkleDiff> {
        let t0 = std::time::Instant::now();
        let hist = self.history.read().unwrap();
        let base_snapshot = hist.get(from_hash)?;
        let current = self.current_snapshot.read().unwrap();

        let mut diff = Self::diff_nodes(&base_snapshot.root, &current.root);
        diff.from_hash = from_hash.to_string();
        diff.to_hash = current.root_hash.clone();
        diff.total_changes = diff.added_files.len() + diff.modified_files.len() + diff.deleted_files.len();
        diff.duration_micros = t0.elapsed().as_micros() as u64;
        Some(diff)
    }

    fn build_tree_recursive(root_path: &Path) -> (MerkleNode, usize, usize, u64) {
        // Collect all file paths using ignore walker respecting .gitignore
        let mut walker = WalkBuilder::new(root_path);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .filter_entry(|entry| {
                let file_name = entry.file_name().to_string_lossy();
                file_name != ".git" && file_name != "target" && file_name != "node_modules"
            });

        let mut file_entries = Vec::new();
        for res in walker.build() {
            if let Ok(entry) = res {
                let path = entry.path().to_path_buf();
                if path.is_file() {
                    file_entries.push(path);
                }
            }
        }

        // Parallel Blake3 hashing across files
        let hashed_files: Vec<(PathBuf, String, u64, u64)> = file_entries
            .par_iter()
            .filter_map(|path| {
                let meta = fs::metadata(path).ok()?;
                let mtime = meta
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let size = meta.len();
                let hash = Self::hash_file(path).ok()?;
                Some((path.clone(), hash, size, mtime))
            })
            .collect();

        // Assemble into hierarchical BTreeMap tree
        let mut root_node = MerkleNode {
            name: root_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "root".to_string()),
            rel_path: String::new(),
            hash: String::new(),
            is_dir: true,
            size: 0,
            modified_secs: 0,
            children: BTreeMap::new(),
        };

        let mut total_files = 0;
        let mut total_bytes = 0;

        for (abs_path, hash, size, mtime) in hashed_files {
            if let Ok(rel) = abs_path.strip_prefix(root_path) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let parts: Vec<&str> = rel_str.split('/').collect();
                Self::insert_node(&mut root_node, &parts, 0, &rel_str, hash, size, mtime);
                total_files += 1;
                total_bytes += size;
            }
        }

        let total_dirs = Self::finalize_hashes(&mut root_node);
        (root_node, total_files, total_dirs, total_bytes)
    }

    fn insert_node(
        current: &mut MerkleNode,
        parts: &[&str],
        idx: usize,
        full_rel_path: &str,
        hash: String,
        size: u64,
        mtime: u64,
    ) {
        if idx >= parts.len() {
            return;
        }

        let name = parts[idx].to_string();
        let is_last = idx == parts.len() - 1;

        if is_last {
            current.children.insert(
                name.clone(),
                MerkleNode {
                    name,
                    rel_path: full_rel_path.to_string(),
                    hash,
                    is_dir: false,
                    size,
                    modified_secs: mtime,
                    children: BTreeMap::new(),
                },
            );
        } else {
            let next_rel = parts[..=idx].join("/");
            let dir_entry = current
                .children
                .entry(name.clone())
                .or_insert_with(|| MerkleNode {
                    name,
                    rel_path: next_rel,
                    hash: String::new(),
                    is_dir: true,
                    size: 0,
                    modified_secs: 0,
                    children: BTreeMap::new(),
                });
            Self::insert_node(dir_entry, parts, idx + 1, full_rel_path, hash, size, mtime);
        }
    }

    fn finalize_hashes(node: &mut MerkleNode) -> usize {
        if !node.is_dir {
            return 0;
        }

        let mut dirs = 1;
        let mut hasher = Hasher::new();
        hasher.update(node.name.as_bytes());

        let mut subtree_size = 0u64;
        let mut max_mtime = 0u64;

        for (child_name, child_node) in &mut node.children {
            dirs += Self::finalize_hashes(child_node);
            hasher.update(child_name.as_bytes());
            hasher.update(child_node.hash.as_bytes());
            subtree_size += child_node.size;
            if child_node.modified_secs > max_mtime {
                max_mtime = child_node.modified_secs;
            }
        }

        node.hash = hasher.finalize().to_hex().to_string();
        node.size = subtree_size;
        node.modified_secs = max_mtime;
        dirs
    }

    pub fn hash_file(path: &Path) -> std::io::Result<String> {
        let mut file = fs::File::open(path)?;
        let mut hasher = Hasher::new();
        std::io::copy(&mut file, &mut hasher)?;
        Ok(hasher.finalize().to_hex().to_string())
    }

    pub fn diff_nodes(old: &MerkleNode, new: &MerkleNode) -> MerkleDiff {
        let mut diff = MerkleDiff::default();
        Self::diff_nodes_rec(old, new, &mut diff);
        diff
    }

    fn diff_nodes_rec(old: &MerkleNode, new: &MerkleNode, diff: &mut MerkleDiff) {
        if old.hash == new.hash {
            // Whole subtree is identical! O(1) early exit.
            return;
        }

        if !old.is_dir && !new.is_dir {
            if old.hash != new.hash {
                diff.modified_files.push(new.rel_path.clone());
            }
            return;
        }

        // Compare children
        for (name, old_child) in &old.children {
            match new.children.get(name) {
                Some(new_child) => {
                    Self::diff_nodes_rec(old_child, new_child, diff);
                }
                None => {
                    Self::collect_all_files(old_child, &mut diff.deleted_files);
                }
            }
        }

        for (name, new_child) in &new.children {
            if !old.children.contains_key(name) {
                Self::collect_all_files(new_child, &mut diff.added_files);
            }
        }
    }

    fn collect_all_files(node: &MerkleNode, out: &mut Vec<String>) {
        if !node.is_dir {
            out.push(node.rel_path.clone());
        } else {
            for child in node.children.values() {
                Self::collect_all_files(child, out);
            }
        }
    }
}
