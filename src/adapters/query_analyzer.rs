use crate::domain::{
    NormalizedQueryIntent, QueryIntentCategory, SearchError,
};
use crate::ports::QueryUnderstandingPort;
use crate::query::{detect_language, extract_temporal};

pub struct NaturalLanguageQueryAnalyzer;

impl Default for NaturalLanguageQueryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl NaturalLanguageQueryAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl QueryUnderstandingPort for NaturalLanguageQueryAnalyzer {
    fn parse_intent(&self, raw_query: &str) -> Result<NormalizedQueryIntent, SearchError> {
        let clean = raw_query.trim();
        let lower = clean.to_lowercase();
        let lang = detect_language(clean);

        let mut intent = NormalizedQueryIntent::new(clean);
        intent.detected_language = Some(lang.to_string());

        // 1. Detect Category
        if lower.contains("lỗi") || lower.contains("error") || lower.contains("crash") || lower.contains("bug") {
            intent.category = QueryIntentCategory::DebugCode;
        } else if lower.contains("so sánh") || lower.contains("compare") || lower.contains("diff") {
            intent.category = QueryIntentCategory::CompareDocuments;
        } else if lower.contains("tìm") || lower.contains("find") || lower.contains("search") || lower.contains("lấy") {
            intent.category = QueryIntentCategory::FindDocument;
        }

        // 2. Extract Mandatory Temporal Filters
        if let Some(temporal) = extract_temporal(clean) {
            intent.mandatory_filter.temporal_month = temporal.month;
            intent.mandatory_filter.temporal_year = temporal.year;
        }

        // 3. Extract File Extension Constraints
        let extensions = ["pdf", "docx", "txt", "md", "json", "ts", "tsx", "rs", "png", "jpg"];
        for ext in extensions {
            let patterns = [
                format!(".{}", ext),
                format!("file {}", ext),
                format!("tệp {}", ext),
                format!("đuôi {}", ext),
                format!("{} file", ext),
            ];
            for p in patterns {
                if lower.contains(&p) {
                    if !intent.mandatory_filter.required_extensions.contains(&ext.to_string()) {
                        intent.mandatory_filter.required_extensions.push(ext.to_string());
                    }
                    break;
                }
            }
        }

        // 4. Extract Negation Constraints (MUST NEVER BE DROPPED)
        // Vietnamese patterns: "không phải [X]", "loại trừ [X]", "không lấy [X]"
        // English patterns: "not [X]", "exclude [X]", "without [X]"
        let negation_indicators = [
            "không phải", "loại trừ", "không lấy", "bỏ qua", "not", "exclude", "without",
        ];
        for ind in negation_indicators {
            if let Some(idx) = lower.find(ind) {
                let after = &lower[idx + ind.len()..];
                let tokens: Vec<&str> = after
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .take(2)
                    .collect();
                if !tokens.is_empty() {
                    let neg_phrase = tokens.join(" ");
                    let clean_neg = neg_phrase.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_string();
                    if !clean_neg.is_empty() && !intent.mandatory_filter.negative_keywords.contains(&clean_neg) {
                        intent.mandatory_filter.negative_keywords.push(clean_neg);
                    }
                }
            }
        }

        // 5. Bounded Semantic Query Expansion (respecting MAX_EXPANDED_TERMS = 20)
        let words: Vec<&str> = lower.split_whitespace().collect();
        for w in words {
            let clean_w = w.trim_matches(|c: char| !c.is_alphanumeric());
            if clean_w.len() > 2 && !intent.expanded_terms.contains(&clean_w.to_string()) {
                intent.expanded_terms.push(clean_w.to_string());
                if intent.expanded_terms.len() >= NormalizedQueryIntent::MAX_EXPANDED_TERMS {
                    break;
                }
            }
        }

        // Validate invariants
        intent.validate()?;

        Ok(intent)
    }
}
