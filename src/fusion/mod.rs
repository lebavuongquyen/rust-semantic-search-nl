use crate::query::ParsedSearchPlan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDocumentMatch {
    pub file_name: String,
    pub final_score: f32,
    pub matched_keywords: Vec<String>,
    pub matched_temporal: Vec<String>,
    pub matched_reasons: Vec<String>,
    pub is_candidate: bool,
}

pub struct HybridScorer;

impl HybridScorer {
    /// Chấm điểm tài liệu kết hợp Ngữ nghĩa (Lexical/Semantic Concept) + Thời gian trong nội dung + Tên file
    pub fn score_document(
        file_name: &str,
        doc_content: &str,
        plan: &ParsedSearchPlan,
    ) -> ScoredDocumentMatch {
        let fn_lower = file_name.to_lowercase();
        let content_lower = doc_content.to_lowercase();

        let mut matched_keywords = Vec::new();
        let mut matched_temporal = Vec::new();
        let mut matched_reasons = Vec::new();

        let mut score = 0.0f32;

        // 1. Kiểm tra khớp Từ khóa & Thực thể mở rộng (Domain Concepts)
        for kw in &plan.semantic_keywords {
            let kw_lower = kw.to_lowercase();
            if fn_lower.contains(&kw_lower) {
                score += 350.0;
                matched_keywords.push(kw.clone());
                matched_reasons.push(format!("Tên file chứa: '{}'", kw));
            } else if content_lower.contains(&kw_lower) {
                score += 200.0;
                matched_keywords.push(kw.clone());
                matched_reasons.push(format!("Nội dung văn bản chứa: '{}'", kw));
            }
        }

        // 2. Kiểm tra khớp Thời gian trong NỘI DUNG (Content-based Temporal Match)
        if let Some(temporal) = &plan.temporal_info {
            for pat in &temporal.patterns {
                let pat_lower = pat.to_lowercase();
                if content_lower.contains(&pat_lower) || fn_lower.contains(&pat_lower) {
                    score += 250.0;
                    matched_temporal.push(pat.clone());
                    matched_reasons.push(format!("Khớp biểu thức thời gian: '{}'", pat));
                    break;
                }
            }
        }

        matched_keywords.sort();
        matched_keywords.dedup();
        matched_temporal.sort();
        matched_temporal.dedup();
        matched_reasons.dedup();

        let is_candidate = score >= 150.0;

        ScoredDocumentMatch {
            file_name: file_name.to_string(),
            final_score: score,
            matched_keywords,
            matched_temporal,
            matched_reasons,
            is_candidate,
        }
    }
}
