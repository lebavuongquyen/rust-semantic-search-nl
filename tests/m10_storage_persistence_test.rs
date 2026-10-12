use semantic_search_agent::adapters::disk_store::DiskDocumentStore;
use semantic_search_agent::adapters::fakes::ReciprocalRankFusion;
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Document, DocumentId, Query};
use semantic_search_agent::ports::{DocumentStore, EmbeddingProvider};
use std::fs;

#[test]
fn test_m10_disk_document_store_atomic_writes_and_recovery() {
    let temp_dir = std::env::temp_dir().join(format!("ssa_test_store_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);

    // Phase 1: Ingest into DiskDocumentStore
    {
        let mut store = DiskDocumentStore::open(&temp_dir).expect("Store opened");
        assert_eq!(store.total_documents(), 0);

        let doc1 = Document::new("doc-01", "Hóa đơn dịch vụ đám mây", "Chi phí cloud tháng 8 là 15,000,000 VND");
        let doc2 = Document::new("doc-02", "Hợp đồng dịch vụ bảo trì", "Bảo trì hệ thống tìm kiếm ngữ nghĩa 2026");

        store.put_document(doc1).expect("Doc 1 stored");
        store.put_document(doc2).expect("Doc 2 stored");

        assert_eq!(store.total_documents(), 2);
        assert!(store.get_document(&DocumentId("doc-01".to_string())).unwrap().is_some());

        // Verify JSON files actually exist on disk
        let doc1_file = temp_dir.join("documents").join("doc-01.json");
        assert!(doc1_file.exists(), "Document JSON must exist on disk");

        // Delete doc2 and verify file removal
        let deleted = store.delete_document(&DocumentId("doc-02".to_string())).expect("Doc 2 deleted");
        assert!(deleted);
        assert!(!temp_dir.join("documents").join("doc-02.json").exists(), "Deleted doc file must be removed");
    }

    // Phase 2: Simulate process crash/restart by reopening store from the same directory
    {
        let store = DiskDocumentStore::open(&temp_dir).expect("Store reopened");
        assert_eq!(store.total_documents(), 1, "Only surviving documents should be recovered");

        let recovered_doc = store.get_document(&DocumentId("doc-01".to_string())).unwrap().expect("Doc 1 found");
        assert_eq!(recovered_doc.title, "Hóa đơn dịch vụ đám mây");
        assert_eq!(recovered_doc.content, "Chi phí cloud tháng 8 là 15,000,000 VND");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_m10_index_snapshots_and_restart_recovery() {
    let temp_dir = std::env::temp_dir().join(format!("ssa_test_index_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let vector_snapshot = temp_dir.join("vector_index.json");
    let lexical_snapshot = temp_dir.join("bm25_index.json");

    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();

    // Phase 1: Ingest and save snapshots
    {
        let store = DiskDocumentStore::open(&temp_dir).expect("Store opened");
        let lexical = Bm25LexicalRetriever::default();
        let vector_index = MemoryVectorIndex::new(space.clone());
        let fusion = ReciprocalRankFusion::default();

        let mut orchestrator = SearchOrchestrator::new(
            store,
            Some(lexical),
            Some(vector_index),
            Some(embedder),
            fusion,
        );

        let doc = Document::new(
            "doc-bidv-receipt",
            "Giấy nộp tiền lệ phí hộ tịch BIDV",
            "GIẤY NỘP TIỀN THU PHÍ LỆ PHÍ HỘ TỊCH NGÂN HÀNG BIDV THÁNG 8 NĂM 2026 SỐ TIỀN 80000",
        );
        orchestrator.ingest_document(doc, 500).expect("Ingested doc");

        // Save index snapshots
        orchestrator.vector_mut().unwrap().save_snapshot(&vector_snapshot).expect("Saved vector snapshot");
        orchestrator.lexical_mut().unwrap().save_snapshot(&lexical_snapshot).expect("Saved BM25 snapshot");

        assert!(vector_snapshot.exists());
        assert!(lexical_snapshot.exists());
    }

    // Phase 2: Restart from disk without re-ingesting documents!
    {
        let store = DiskDocumentStore::open(&temp_dir).expect("Store reopened");
        let restored_vector = MemoryVectorIndex::load_snapshot(&vector_snapshot).expect("Loaded vector snapshot");
        let restored_lexical = Bm25LexicalRetriever::load_snapshot(&lexical_snapshot).expect("Loaded BM25 snapshot");
        let embedder = MultilingualSemanticEmbedder::new();
        let fusion = ReciprocalRankFusion::default();

        let orchestrator = SearchOrchestrator::new(
            store,
            Some(restored_lexical),
            Some(restored_vector),
            Some(embedder),
            fusion,
        );

        let query = Query::new("biên lai thu tiền lệ phí").with_top_k(3);
        let results = orchestrator.search(&query).expect("Search executed on restored orchestrator");

        assert!(!results.is_empty(), "Restored orchestrator must return search results");
        assert_eq!(results[0].document_id.0, "doc-bidv-receipt");
        assert!(results[0].score.final_score > 0.0);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
