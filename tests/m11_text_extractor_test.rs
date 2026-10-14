use semantic_search_agent::adapters::text_extractor::{MultimodalMockExtractor, PlainTextExtractor};
use semantic_search_agent::ports::TextExtractorPort;

#[test]
fn test_m11_plaintext_extractor_formats_and_metadata() {
    let extractor = PlainTextExtractor;
    let content_bytes = b"# Architecture Overview\nThis document describes the hexagonal search core.";
    let extracted = extractor
        .extract_text("docs/ARCHITECTURE.md", content_bytes)
        .expect("Extraction should succeed");

    assert_eq!(extracted.title, Some("ARCHITECTURE".to_string()));
    assert_eq!(extracted.mime_type, "text/markdown");
    assert_eq!(extracted.confidence, Some(1.0));
    assert_eq!(extracted.extraction_method, "plain_text");
    assert!(extracted.text.contains("hexagonal search core"));
    assert_eq!(
        extracted.metadata.get("byte_length").map(|s| s.as_str()),
        Some(content_bytes.len().to_string().as_str())
    );
}

#[test]
fn test_m11_multimodal_ocr_extractor_for_scanned_documents() {
    let extractor = MultimodalMockExtractor;
    assert!(extractor.supported_formats().contains(&"pdf"));
    assert!(extractor.supported_formats().contains(&"png"));

    // Simulate scanned PDF byte stream containing embedded text cues
    let scanned_pdf_bytes = b"%PDF-1.4\n1 0 obj\n<< /Title (Invoice 2026-08) >>\nstream\nHoadon thanh toan dich vu cloud thang 08\nendstream";
    let extracted = extractor
        .extract_text("scans/hoadon_202608.pdf", scanned_pdf_bytes)
        .expect("OCR extraction should succeed");

    assert_eq!(extracted.title, Some("hoadon_202608".to_string()));
    assert_eq!(extracted.mime_type, "application/pdf");
    assert_eq!(extracted.confidence, Some(0.92));
    assert_eq!(extracted.extraction_method, "multimodal_ocr");
    assert!(extracted.text.contains("Invoice"));
    assert!(extracted.text.contains("Hoadon thanh toan"));
}

#[test]
fn test_m11_multimodal_extractor_empty_bytes_rejected() {
    let extractor = MultimodalMockExtractor;
    let err = extractor.extract_text("empty.pdf", b"").unwrap_err();
    assert!(err.to_string().contains("Cannot extract text from empty document"));
}
