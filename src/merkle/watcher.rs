use super::tree::{MerkleDiff, MerkleTree, MerkleTreeSnapshot};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FileWatcher {
    merkle_tree: Arc<MerkleTree>,
    dirty_files: Arc<std::sync::Mutex<HashSet<String>>>,
}

impl FileWatcher {
    pub fn new(root_path: PathBuf) -> Self {
        let tree = Arc::new(MerkleTree::new(root_path));
        Self {
            merkle_tree: tree,
            dirty_files: Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }

    pub fn tree(&self) -> Arc<MerkleTree> {
        self.merkle_tree.clone()
    }

    pub fn get_snapshot(&self) -> MerkleTreeSnapshot {
        self.merkle_tree.get_snapshot()
    }

    pub fn mark_dirty(&self, rel_path: &str) {
        let mut dirty = self.dirty_files.lock().unwrap();
        dirty.insert(rel_path.replace('\\', "/"));
    }

    pub fn take_dirty_files(&self) -> Vec<String> {
        let mut dirty = self.dirty_files.lock().unwrap();
        let list: Vec<String> = dirty.iter().cloned().collect();
        dirty.clear();
        list
    }

    pub fn sync_and_diff(&self) -> MerkleDiff {
        let diff = self.merkle_tree.refresh();
        let mut dirty = self.dirty_files.lock().unwrap();
        for f in &diff.added_files {
            dirty.insert(f.clone());
        }
        for f in &diff.modified_files {
            dirty.insert(f.clone());
        }
        for f in &diff.deleted_files {
            dirty.remove(f);
        }
        diff
    }

    pub fn diff_since_hash(&self, from_hash: &str) -> Option<MerkleDiff> {
        self.merkle_tree.diff_against_hash(from_hash)
    }

    pub fn root_hash(&self) -> String {
        self.merkle_tree.current_root_hash()
    }
}
