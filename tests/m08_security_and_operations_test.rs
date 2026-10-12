use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Document, Query};
use semantic_search_agent::security::{AccessControlPolicy, TelemetryRedactionPolicy};

#[test]
fn test_m08_access_control_tenant_isolation() {
    let policy = AccessControlPolicy::new("tenant-alpha", vec!["user".to_string()]);

    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::<
        InMemoryDocumentStore,
        Bm25LexicalRetriever,
        semantic_search_agent::adapters::fakes::FakeVectorRetriever,
        semantic_search_agent::adapters::fakes::FakeEmbeddingProvider,
        ReciprocalRankFusion,
    >::new(store, Some(bm25), None, None, fusion);

    let doc_alpha = Document::new("doc-a", "Alpha Secret Report", "Alpha confidential data in secret report")
        .with_metadata("tenant_id", "tenant-alpha");
    let doc_beta = Document::new("doc-b", "Beta Secret Report", "Beta confidential data in secret report")
        .with_metadata("tenant_id", "tenant-beta");
    let doc_no_tenant = Document::new("doc-unassigned", "Public Secret Report", "Document without tenant metadata in secret report");
    let doc_alpha_admin = Document::new("doc-admin", "Alpha Admin Only Report", "Executive confidential secret report")
        .with_metadata("tenant_id", "tenant-alpha")
        .with_metadata("required_roles", "admin");

    orchestrator.ingest_document(doc_alpha, 200).unwrap();
    orchestrator.ingest_document(doc_beta, 200).unwrap();
    orchestrator.ingest_document(doc_no_tenant, 200).unwrap();
    orchestrator.ingest_document(doc_alpha_admin, 200).unwrap();

    let raw_results = orchestrator.search(&Query::new("secret report")).unwrap();
    assert_eq!(raw_results.len(), 4, "All four documents should be retrieved before access control");

    let authorized_results = policy.filter_authorized_results(raw_results);

    // Fail closed:
    // - doc-b rejected (mismatched tenant: tenant-beta != tenant-alpha)
    // - doc-unassigned rejected (missing tenant_id)
    // - doc-admin rejected (requires role 'admin', user only has 'user')
    // - doc-a allowed (matches tenant-alpha, no restrictive role)
    assert_eq!(authorized_results.len(), 1, "Only doc-a should pass fail-closed tenant and role authorization");
    assert_eq!(authorized_results[0].document_id.0, "doc-a");
}

#[test]
fn test_m08_secret_redaction_in_telemetry() {
    let sensitive_log = "Sending request to Google AI Studio with key=AIzaSyB1234567890abcdefghijklmnopqrstuv and Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.fake_signature";
    let redacted = TelemetryRedactionPolicy::redact(sensitive_log);

    assert!(!redacted.contains("AIzaSyB1234567890abcdefghijklmnopqrstuv"));
    assert!(!redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    assert!(redacted.contains("[REDACTED_API_KEY]") || redacted.contains("key=[REDACTED]"));
    assert!(redacted.contains("Bearer [REDACTED_TOKEN]"));
}

#[test]
fn test_m08_deletion_propagation_cleans_up_index() {
    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let fusion = ReciprocalRankFusion::default();

    let space = semantic_search_agent::adapters::fakes::FakeEmbeddingProvider::standard_test_space();
    let vector = semantic_search_agent::adapters::fakes::FakeVectorRetriever::new(space.clone());
    let embedder = semantic_search_agent::adapters::fakes::FakeEmbeddingProvider::new(space);

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector),
        Some(embedder),
        fusion,
    );

    let doc = Document::new("doc-to-delete", "Invoice Temporary", "Temporary invoice 100 USD");
    let doc_id = doc.id.clone();

    orchestrator.ingest_document(doc, 200).unwrap();

    // Verify document is searchable before deletion
    let before_results = orchestrator.search(&Query::new("temporary invoice")).unwrap();
    assert_eq!(before_results.len(), 1);

    // Propagate deletion
    let deleted = orchestrator.delete_document(&doc_id).unwrap();
    assert!(deleted, "Document must be deleted successfully");

    // Verify document is NOT returned in search results after deletion
    let after_results = orchestrator.search(&Query::new("temporary invoice")).unwrap();
    assert!(after_results.is_empty(), "Deleted document must not appear in search results");
}

#[test]
fn test_m08_ingestion_edge_cases_and_reingestion() {
    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let fusion = ReciprocalRankFusion::default();
    let space = semantic_search_agent::adapters::fakes::FakeEmbeddingProvider::standard_test_space();
    let vector = semantic_search_agent::adapters::fakes::FakeVectorRetriever::new(space.clone());
    let embedder = semantic_search_agent::adapters::fakes::FakeEmbeddingProvider::new(space);

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector),
        Some(embedder),
        fusion,
    );

    // 1. Zero chunk size must be rejected explicitly
    let doc_zero = Document::new("doc-z", "Zero Doc", "Some valid content");
    let res_zero = orchestrator.ingest_document(doc_zero, 0);
    assert!(res_zero.is_err(), "Must reject chunk_size == 0");

    // 2. Multibyte UTF-8 Vietnamese boundary safety
    // Tiếng Việt with combining diacritics
    let doc_vi = Document::new("doc-vi", "Biên nhận", "Hóa đơn dịch vụ giá trị gia tăng và biên lai thu lệ phí");
    // Ingest with tiny chunk_size in characters (e.g. 5 chars) which would panic if sliced by raw bytes
    let chunks_vi = orchestrator.ingest_document(doc_vi, 5).expect("UTF-8 boundary chunking must succeed");
    assert!(chunks_vi.len() > 1);

    // 3. Re-ingestion reconciles prior chunks
    let doc_update = Document::new("doc-vi", "Biên nhận mới", "Nội dung cập nhật hoàn toàn mới");
    let chunks_new = orchestrator.ingest_document(doc_update, 100).expect("Re-ingestion must succeed");
    assert_eq!(chunks_new.len(), 1);

    let results = orchestrator.search(&Query::new("cập nhật")).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document_id.0, "doc-vi");
}
