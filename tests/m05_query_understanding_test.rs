use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::query_analyzer::NaturalLanguageQueryAnalyzer;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Document, NormalizedQueryIntent, QueryIntentCategory};
use semantic_search_agent::ports::{EmbeddingProvider, QueryUnderstandingPort};

#[test]
fn test_m05_intent_parsing_and_constraint_preservation() {
    let analyzer = NaturalLanguageQueryAnalyzer::new();

    // Query with negation and file extension: "tìm hóa đơn dịch vụ file pdf loại trừ draft"
    let raw = "tìm hóa đơn dịch vụ file pdf loại trừ draft";
    let intent = analyzer.parse_intent(raw).expect("Parsed intent");

    assert_eq!(intent.category, QueryIntentCategory::FindDocument);
    assert_eq!(intent.detected_language.as_deref(), Some("vi"));

    // Check mandatory extension filter
    assert!(intent.mandatory_filter.required_extensions.contains(&"pdf".to_string()));

    // Check mandatory negative filter: MUST NOT BE DROPPED
    assert!(intent.mandatory_filter.negative_keywords.contains(&"draft".to_string()));

    // Invariant validation succeeds
    assert!(intent.validate().is_ok());
}

#[test]
fn test_m05_invariant_rejection_when_negation_dropped() {
    let mut bad_intent = NormalizedQueryIntent::new("tìm hợp đồng loại trừ draft");
    // Manually clearing negative keywords while query has negation cue
    bad_intent.mandatory_filter.negative_keywords.clear();

    let err = bad_intent.validate();
    assert!(err.is_err(), "Must reject query intent when negation constraint was dropped");
}

#[test]
fn test_m05_search_orchestrator_with_intent() {
    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector_index),
        Some(embedder),
        fusion,
    );

    let doc1 = Document::new("doc-valid", "Invoice Consulting", "Consulting services invoice 4500 USD")
        .with_file_path("finance/invoice.pdf");
    let doc2 = Document::new("doc-draft", "Invoice Draft Template", "Draft invoice template")
        .with_file_path("finance/invoice_draft.pdf");

    orchestrator.ingest_document(doc1, 200).unwrap();
    orchestrator.ingest_document(doc2, 200).unwrap();

    let analyzer = NaturalLanguageQueryAnalyzer::new();
    let intent = analyzer.parse_intent("tìm hóa đơn dịch vụ file pdf không phải draft").unwrap();

    let results = orchestrator.search_with_intent(&intent, 5).unwrap();

    assert_eq!(results.len(), 1, "Draft should be strictly excluded by mandatory filter");
    assert_eq!(results[0].document_id.0, "doc-valid");
}
