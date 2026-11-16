use antigravity_fast_core::{CrepeIndex, SparseNgramExtractor, Verifier};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_sparse_ngram_extraction() {
    let sample = b"fn authenticate_user(token: &str) -> Result<User, AuthError> { Ok(User) }";
    let ngrams = SparseNgramExtractor::extract_all(sample);
    assert!(!ngrams.is_empty(), "Sparse n-grams should be extracted");

    let query_ngrams = SparseNgramExtractor::extract_query_ngrams("authenticate_user");
    assert!(!query_ngrams.is_empty(), "Query n-grams should be extracted");
}

#[test]
fn test_crepe_indexing_and_candidate_discovery() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();

    let auth_file = src.join("auth.rs");
    let math_file = src.join("math.rs");
    let net_file = src.join("net.rs");

    fs::write(&auth_file, "pub fn authenticate_token() { verify_jwt(); }\n").unwrap();
    fs::write(&math_file, "pub fn compute_sum(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
    fs::write(&net_file, "pub fn establish_socket_connection() { connect(); }\n").unwrap();

    let crepe = CrepeIndex::new(root.clone());
    let meta = crepe.build().expect("Index build failed");

    assert_eq!(meta.total_files, 3);
    assert!(meta.total_ngrams > 0);

    // 1. Literal query
    let candidates = crepe.find_candidates("authenticate_token");
    assert!(!candidates.is_empty(), "Should find candidate files for query");
    assert!(candidates.iter().any(|c| c.contains("auth.rs")));
    assert!(!candidates.iter().any(|c| c.contains("math.rs")), "Math should be pruned");

    let verifier = Verifier::new(root.clone());
    let matches = verifier
        .verify_in_process("authenticate_token", &candidates, true)
        .expect("Verification failed");

    assert_eq!(matches.len(), 1);
    assert!(matches[0].file.contains("auth.rs"));
    assert!(matches[0].line_content.contains("pub fn authenticate_token"));

    // 2. Case-insensitive query
    let ci_candidates = crepe.find_candidates("AUTHENTICATE_TOKEN");
    assert!(ci_candidates.iter().any(|c| c.contains("auth.rs")), "Must match case-insensitively");
    let ci_matches = verifier
        .verify_in_process("AUTHENTICATE_TOKEN", &ci_candidates, false)
        .expect("CI verification failed");
    assert_eq!(ci_matches.len(), 1);

    // 3. Word boundary regex query
    let wb_candidates = crepe.find_candidates(r"\bcompute_sum\b");
    assert!(wb_candidates.iter().any(|c| c.contains("math.rs")), "Must match regex with word boundaries");
    let wb_matches = verifier
        .verify_in_process(r"\bcompute_sum\b", &wb_candidates, true)
        .expect("WB verification failed");
    assert_eq!(wb_matches.len(), 1);

    // 4. Regex alternation query
    let alt_candidates = crepe.find_candidates("authenticate_token|compute_sum");
    assert!(alt_candidates.iter().any(|c| c.contains("auth.rs")), "Alternation branch 1");
    assert!(alt_candidates.iter().any(|c| c.contains("math.rs")), "Alternation branch 2");
    let alt_matches = verifier
        .verify_in_process("authenticate_token|compute_sum", &alt_candidates, true)
        .expect("Alt verification failed");
    assert_eq!(alt_matches.len(), 2);
}
