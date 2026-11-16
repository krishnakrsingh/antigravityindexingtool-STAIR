use antigravity_fast_core::MerkleTree;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_merkle_tree_construction_and_diff() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create file structure
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file_a = src_dir.join("a.rs");
    let file_b = src_dir.join("b.rs");

    fs::write(&file_a, "fn hello_world() {}\n").unwrap();
    fs::write(&file_b, "struct User { id: u64 }\n").unwrap();

    let tree = MerkleTree::new(root.clone());
    let snap1 = tree.get_snapshot();

    assert_eq!(snap1.total_files, 2);
    assert!(!snap1.root_hash.is_empty());

    // Clean refresh with no changes -> 0 diff
    let diff0 = tree.refresh();
    assert_eq!(diff0.total_changes, 0);
    assert_eq!(diff0.from_hash, diff0.to_hash);

    // Modify file_a
    fs::write(&file_a, "fn hello_world() { println!(\"updated\"); }\n").unwrap();

    let diff1 = tree.refresh();
    assert_eq!(diff1.total_changes, 1);
    assert_eq!(diff1.modified_files.len(), 1);
    assert!(diff1.modified_files[0].contains("a.rs"));
    assert_ne!(diff1.from_hash, diff1.to_hash);

    // Add a new file
    let file_c = src_dir.join("c.rs");
    fs::write(&file_c, "pub enum Status { Active, Idle }\n").unwrap();

    let diff2 = tree.refresh();
    assert_eq!(diff2.added_files.len(), 1);
    assert!(diff2.added_files[0].contains("c.rs"));

    // Delete a file
    fs::remove_file(&file_b).unwrap();
    let diff3 = tree.refresh();
    assert_eq!(diff3.deleted_files.len(), 1);
    assert!(diff3.deleted_files[0].contains("b.rs"));
}
