use crate::domain::SearchError;
use crate::ports::{ExtractedContent, TextExtractorPort};
use std::collections::HashMap;

/// Standard text extractor handling common UTF-8 plaintext and code file formats:
/// .txt, .md, .markdown, .json, .rs, .ts, .js, .csv, .yaml, .yml
pub struct PlainTextExtractor;

impl TextExtractorPort for PlainTextExtractor {
    fn supported_formats(&self) -> &[&str] {
        &[
            "txt", "md", "markdown", "json", "rs", "ts", "js", "csv", "yaml", "yml", "toml", "xml",
            "text/plain", "text/markdown", "application/json",
        ]
    }

    fn extract_text(&self, file_name: &str, bytes: &[u8]) -> Result<ExtractedContent, SearchError> {
        let text = match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                // Lossy fallback for Latin/extended encodings
                String::from_utf8_lossy(bytes).to_string()
            }
        };

        let mut metadata = HashMap::new();
        metadata.insert("file_name".to_string(), file_name.to_string());
        metadata.insert("byte_length".to_string(), bytes.len().to_string());

        let title = std::path::Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        let ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_lowercase();

        let mime = match ext.as_str() {
            "md" | "markdown" => "text/markdown",
            "json" => "application/json",
            "rs" | "ts" | "js" => "text/code",
            "csv" => "text/csv",
            _ => "text/plain",
        };

        Ok(ExtractedContent {
            text,
            title,
            mime_type: mime.to_string(),
            confidence: Some(1.0),
            extraction_method: "plain_text".to_string(),
            metadata,
        })
    }
}

/// Simulated multimodal / OCR extractor double for scanned files (.pdf, .png, .jpg)
/// In production, can delegate to local tesseract-sys or Google Cloud Vision API.
pub struct MultimodalMockExtractor;

impl TextExtractorPort for MultimodalMockExtractor {
    fn supported_formats(&self) -> &[&str] {
        &["pdf", "png", "jpg", "jpeg", "application/pdf", "image/png", "image/jpeg"]
    }

    fn extract_text(&self, file_name: &str, bytes: &[u8]) -> Result<ExtractedContent, SearchError> {
        if bytes.is_empty() {
            return Err(SearchError::InvalidConfiguration(format!(
                "Cannot extract text from empty document: {}",
                file_name
            )));
        }

        // Check if raw bytes contain embedded text (e.g. text layer in PDF)
        let lossy = String::from_utf8_lossy(bytes);
        let mut extracted_text = String::new();

        // Extract any printable ASCII or UTF-8 chunks
        let mut current_word = String::new();
        for c in lossy.chars() {
            if c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '_' || c == ':' || c == '.' {
                current_word.push(c);
            } else if !current_word.trim().is_empty() {
                if current_word.trim().len() > 2 {
                    extracted_text.push_str(current_word.trim());
                    extracted_text.push(' ');
                }
                current_word.clear();
            }
        }
        if !current_word.trim().is_empty() {
            extracted_text.push_str(current_word.trim());
        }

        // If no text layer was discoverable, generate descriptive metadata extraction
        if extracted_text.trim().is_empty() {
            extracted_text = format!("Scanned multimodal asset: {} ({} bytes)", file_name, bytes.len());
        }

        let mut metadata = HashMap::new();
        metadata.insert("file_name".to_string(), file_name.to_string());
        metadata.insert("is_multimodal".to_string(), "true".to_string());

        let title = std::path::Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        Ok(ExtractedContent {
            text: extracted_text,
            title,
            mime_type: if file_name.ends_with(".pdf") { "application/pdf".to_string() } else { "image/scanned".to_string() },
            confidence: Some(0.92),
            extraction_method: "multimodal_ocr".to_string(),
            metadata,
        })
    }
}
