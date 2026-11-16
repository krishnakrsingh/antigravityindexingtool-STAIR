use antigravity_fast_core::LocalSemanticIndex;

#[test]
fn test_local_semantic_index_indexing_and_search() {
    let index = LocalSemanticIndex::new();

    let items = vec![
        (
            "billing/reconciler.rs::GlobalSovereignLedger".to_string(),
            "pub struct GlobalSovereignLedger".to_string(),
            "billing ledger sovereign assets liabilities settlement reconciler".to_string(),
        ),
        (
            "billing/reconciler.rs::execute_atomic_settlement".to_string(),
            "pub async fn execute_atomic_settlement(tx_id: &str, amount: i64) -> Result<String, String>".to_string(),
            "atomic financial settlement proof execution transaction".to_string(),
        ),
        (
            "auth/jwt.py::verify_cryptographic_session_token".to_string(),
            "def verify_cryptographic_session_token(token: str, key: str) -> dict".to_string(),
            "cryptographic jwt authentication bearer token rsa session".to_string(),
        ),
        (
            "frontend/PaymentPortal.ts::PaymentOrchestrationController".to_string(),
            "export class PaymentOrchestrationController".to_string(),
            "payment orchestration dispatch transaction gateway endpoint".to_string(),
        ),
    ];

    index.index_symbols_batch(items);
    assert_eq!(index.total_indexed(), 4);

    // Test 1: Exact / close identifier query
    let results = index.search("execute_atomic_settlement", 3);
    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_path, "billing/reconciler.rs::execute_atomic_settlement");
    assert!(results[0].score > 0.5);

    // Test 2: Natural language intent query
    let results = index.search("settle atomic transaction proof", 3);
    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_path, "billing/reconciler.rs::execute_atomic_settlement");

    // Test 3: Sub-token camelCase query
    let results = index.search("payment orchestration gateway", 3);
    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_path, "frontend/PaymentPortal.ts::PaymentOrchestrationController");

    // Test 4: Unrelated query returns nothing above threshold
    let results = index.search("astrophysics planetary telescope", 3);
    assert!(results.is_empty());

    // Test 5: Single symbol indexing
    index.index_symbol(
        "infra/redis.rs::acquire_distributed_lock",
        "pub fn acquire_distributed_lock(key: &str) -> bool",
        "distributed lock mutex redis concurrency",
    );
    assert_eq!(index.total_indexed(), 5);

    let results = index.search("distributed lock mutex", 3);
    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_path, "infra/redis.rs::acquire_distributed_lock");
}
