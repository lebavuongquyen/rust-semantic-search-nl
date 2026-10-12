use semantic_search_agent::adapters::fakes::{
    FakeEmbeddingProvider, FakeLexicalRetriever, FakeVectorRetriever, InMemoryDocumentStore,
    ReciprocalRankFusion,
};
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{
    Document, Query, QueryFilter, SearchError,
};
use semantic_search_agent::ports::{EmbeddingProvider, VectorRetriever};

#[test]
fn test_m01_end_to_end_search_flow_with_test_doubles() {
    let store = InMemoryDocumentStore::new();
    let lexical = FakeLexicalRetriever::new();
    let space = FakeEmbeddingProvider::standard_test_space();
    let vector = FakeVectorRetriever::new(space.clone());
    let embedder = FakeEmbeddingProvider::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(lexical),
        Some(vector),
        Some(embedder),
        fusion,
    );

    // Ingest test documents
    let doc1 = Document::new("doc-1", "Invoice August 2026", "Payment slip for tax and fee 80000 VND")
        .with_file_path("finance/invoice_august_2026.pdf");
    let doc2 = Document::new("doc-2", "Auth Service Specs", "Detailed design of login and session JWT tokens")
        .with_file_path("specs/auth_service.md");
    let doc3 = Document::new("doc-3", "General Guidelines", "Coding conventions and style guide")
        .with_file_path("docs/guidelines.md");

    orchestrator.ingest_document(doc1, 200).expect("Doc 1 ingested");
    orchestrator.ingest_document(doc2, 200).expect("Doc 2 ingested");
    orchestrator.ingest_document(doc3, 200).expect("Doc 3 ingested");

    // Search query for invoice/payment
    let query = Query::new("invoice payment tax");
    let results = orchestrator.search(&query).expect("Search succeeded");

    assert!(!results.is_empty(), "Should return candidates");
    assert_eq!(results[0].document_id.0, "doc-1");
    assert_eq!(results[0].document_title, "Invoice August 2026");
    assert!(!results[0].score.components.is_empty());
    assert_eq!(results[0].provenance.document_id.0, "doc-1");
}

#[test]
fn test_m01_hard_constraint_filtering() {
    let store = InMemoryDocumentStore::new();
    let lexical = FakeLexicalRetriever::new();
    let space = FakeEmbeddingProvider::standard_test_space();
    let vector = FakeVectorRetriever::new(space.clone());
    let embedder = FakeEmbeddingProvider::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(lexical),
        Some(vector),
        Some(embedder),
        fusion,
    );

    let doc1 = Document::new("doc-1", "Invoice PDF", "Payment receipt")
        .with_file_path("finance/inv.pdf");
    let doc2 = Document::new("doc-2", "Invoice TXT Draft", "Payment receipt draft")
        .with_file_path("finance/inv.txt");

    orchestrator.ingest_document(doc1, 200).unwrap();
    orchestrator.ingest_document(doc2, 200).unwrap();

    // 1. Extension filter: only .pdf
    let query_pdf = Query::new("receipt").with_filter(QueryFilter {
        required_extensions: vec!["pdf".to_string()],
        ..Default::default()
    });
    let results_pdf = orchestrator.search(&query_pdf).unwrap();
    assert_eq!(results_pdf.len(), 1);
    assert_eq!(results_pdf[0].document_id.0, "doc-1");

    // 2. Negative keyword filter: exclude "draft"
    let query_no_draft = Query::new("receipt").with_filter(QueryFilter {
        negative_keywords: vec!["draft".to_string()],
        ..Default::default()
    });
    let results_no_draft = orchestrator.search(&query_no_draft).unwrap();
    assert_eq!(results_no_draft.len(), 1);
    assert_eq!(results_no_draft[0].document_id.0, "doc-1");

    // 3. Tag constraint: require category tag "invoice"
    let doc3 = Document::new("doc-3", "Invoice 2025", "Valid invoice receipt")
        .with_metadata("tags", "finance,invoice")
        .with_metadata("year", "2025")
        .with_metadata("month", "5");
    orchestrator.ingest_document(doc3, 200).unwrap();

    let query_tagged = Query::new("receipt").with_filter(QueryFilter {
        required_tags: vec!["invoice".to_string()],
        ..Default::default()
    });
    let results_tagged = orchestrator.search(&query_tagged).unwrap();
    assert_eq!(results_tagged.len(), 1);
    assert_eq!(results_tagged[0].document_id.0, "doc-3");

    // 4. Temporal constraints: year 2025 and month 5
    let query_temporal_ok = Query::new("receipt").with_filter(QueryFilter {
        temporal_year: Some(2025),
        temporal_month: Some(5),
        ..Default::default()
    });
    let results_temporal_ok = orchestrator.search(&query_temporal_ok).unwrap();
    assert_eq!(results_temporal_ok.len(), 1);
    assert_eq!(results_temporal_ok[0].document_id.0, "doc-3");

    // Negative temporal constraint: year 2026 (should return empty)
    let query_temporal_bad = Query::new("receipt").with_filter(QueryFilter {
        temporal_year: Some(2026),
        ..Default::default()
    });
    let results_temporal_bad = orchestrator.search(&query_temporal_bad).unwrap();
    assert!(results_temporal_bad.is_empty(), "Mismatching year must yield zero results");
}

#[test]
fn test_m01_incompatible_embedding_space_fails_explicitly() {
    let space_a = FakeEmbeddingProvider::standard_test_space();
    let mut space_b = space_a.clone();
    space_b.model_id = "completely-different-model".to_string();

    let mut vector_retriever = FakeVectorRetriever::new(space_a);
    let embedder_b = FakeEmbeddingProvider::new(space_b);

    let vec_b = embedder_b.embed_text("test text").unwrap();
    let chunk_id = semantic_search_agent::domain::ChunkId("c1".to_string());

    let result = vector_retriever.index_vector(&chunk_id, &vec_b);
    assert!(result.is_err(), "Must reject incompatible embedding space");
    match result.unwrap_err() {
        SearchError::IncompatibleEmbeddingSpace { expected, actual } => {
            assert!(expected.contains("deterministic-mock-v1"));
            assert!(actual.contains("completely-different-model"));
        }
        other => panic!("Expected IncompatibleEmbeddingSpace, got {:?}", other),
    }
}
