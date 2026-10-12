pub mod adapters;
pub mod agent;
pub mod application;
pub mod domain;
pub mod evaluation;
pub mod fusion;
pub mod ports;
pub mod query;
pub mod security;
pub mod similarity;
pub mod thesaurus;

use fusion::HybridScorer;
use query::{detect_language, extract_temporal, generate_code_naming_patterns, normalize_typos, ParsedSearchPlan};
use thesaurus::ConceptRegistry;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct SemanticSearchAgent {
    thesaurus: ConceptRegistry,
}

#[wasm_bindgen]
impl SemanticSearchAgent {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        Self {
            thesaurus: ConceptRegistry::new(),
        }
    }

    /// Phân tích câu hỏi tự nhiên thành Kế hoạch tìm kiếm ngữ nghĩa toàn diện (Search Plan)
    #[wasm_bindgen(js_name = parseQuery)]
    pub fn parse_query(&self, raw_query: &str) -> Result<JsValue, JsValue> {
        let plan = self.parse_query_internal(raw_query);
        serde_wasm_bindgen::to_value(&plan).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Chấm điểm và xếp hạng tài liệu dựa trên nội dung & Kế hoạch tìm kiếm
    #[wasm_bindgen(js_name = scoreDocument)]
    pub fn score_document(
        &self,
        file_name: &str,
        doc_content: &str,
        plan_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let plan: ParsedSearchPlan = serde_wasm_bindgen::from_value(plan_js)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let match_result = HybridScorer::score_document(file_name, doc_content, &plan);
        serde_wasm_bindgen::to_value(&match_result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Tính Cosine Similarity giữa 2 vector embedding
    #[wasm_bindgen(js_name = computeCosineSimilarity)]
    pub fn compute_cosine_similarity(&self, vec_a: &[f32], vec_b: &[f32]) -> f32 {
        similarity::cosine_similarity(vec_a, vec_b)
    }
}

impl Default for SemanticSearchAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticSearchAgent {
    pub fn parse_query_internal(&self, raw_query: &str) -> ParsedSearchPlan {
        let lang = detect_language(raw_query);
        let normalized = normalize_typos(raw_query);

        // Mở rộng thực thể qua Concept Registry
        let mut semantic_keywords = self.thesaurus.expand_term(&normalized);
        if semantic_keywords.is_empty() {
            semantic_keywords.push(normalized.clone());
            // Tách các từ/cụm từ con để không bị chặn cứng khi tìm kiếm chuẩn
            let parts: Vec<&str> = normalized.split_whitespace().collect();
            if parts.len() > 1 {
                for chunk in parts.chunks(2) {
                    semantic_keywords.push(chunk.join(" "));
                }
                for p in parts {
                    if p.len() > 2 {
                        semantic_keywords.push(p.to_string());
                    }
                }
            }
        }

        // Sinh mẫu tên tệp và routes cho Coding Agent
        let (name_patterns, code_routes) = generate_code_naming_patterns(&semantic_keywords);

        // Trích xuất biểu thức thời gian
        let temporal_info = extract_temporal(raw_query);

        ParsedSearchPlan {
            provider: "standard_pure".to_string(),
            raw_query: raw_query.to_string(),
            language: lang.to_string(),
            normalized_query: normalized,
            semantic_keywords,
            name_patterns,
            code_routes,
            temporal_info,
            target_extensions: Vec::new(),
            negative_keywords: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bidv_receipt_catalyst_case() {
        let agent = SemanticSearchAgent::new();

        // Ca kiểm thử của người dùng: Gõ nhầm "biên lại", tìm "tháng 8"
        let user_query = "tìm biên lại thu tiền tháng 8";
        let plan = agent.parse_query_internal(user_query);

        assert_eq!(plan.language, "vi");
        assert!(plan.normalized_query.contains("biên lai"));
        // Phải mở rộng được ra "giấy nộp tiền", "lệ phí", "chứng từ"
        assert!(plan.semantic_keywords.contains(&"giấy nộp tiền".to_string()));
        assert!(plan.semantic_keywords.contains(&"lệ phí".to_string()));

        // Phải bắt được tháng 8 và mẫu "tháng 08"
        assert!(plan.temporal_info.is_some());
        let temporal = plan.temporal_info.as_ref().unwrap();
        assert_eq!(temporal.month, Some(8));
        assert!(temporal.patterns.contains(&"tháng 08".to_string()));

        // Văn bản thực tế của tờ chứng từ BIDV
        let doc_content = r#"
GIẤY NỘP TIỀN THU PHÍ, LỆ PHÍ VÀ THU PHẠT VI PHẠM HÀNH CHÍNH
Mã hồ sơ: H29.243-260824-150060
Tên phí, lệ phí: Lệ phí hộ tịch
Người nộp: Lê Bá Vương Quyến
Địa chỉ: Xã Bà Điểm, Thành phố Hồ Chí Minh
Số tiền: 80,000 VND
Người nộp tiền - Ngày 24 Tháng 08 Năm 2026
"#;

        let result = HybridScorer::score_document("H29.243-260824-150060.pdf", doc_content, &plan);

        // Phải là ứng viên hợp lệ với điểm số cao
        assert!(result.is_candidate);
        assert!(result.final_score > 400.0);
        println!("Test Passed! Score: {}, Reasons: {:?}", result.final_score, result.matched_reasons);
    }

    #[test]
    fn test_coding_agent_login_error_case() {
        let agent = SemanticSearchAgent::new();

        let query = "lỗi không đăng nhập được";
        let plan = agent.parse_query_internal(query);

        assert_eq!(plan.language, "vi");
        assert!(plan.code_routes.contains(&"/dang-nhap".to_string()));
        assert!(plan.code_routes.contains(&"/api/dang-nhap".to_string()));
        assert!(plan.code_routes.contains(&"/login".to_string()));
    }

    #[test]
    fn test_workspace_files_guidance_documentation() {
        use std::fs;

        let agent = SemanticSearchAgent::new();

        // 1. Trường hợp 1: Chạy Flow Chuẩn nội bộ (Standard Pure Engine)
        let query = "tài liệu hướng dẫn";
        let standard_plan = agent.parse_query_internal(query);

        // 2. Trường hợp 2: Mô phỏng Search Plan sinh bởi Google AI Studio (Cross-lingual Semantic Expansion)
        // Khi người dùng hỏi "tài liệu hướng dẫn", Gemini Flash suy luận mở rộng ra:
        // keywords: ["tài liệu", "hướng dẫn", "readme", "guide", "guidelines", "specification", "manual", "docs", "documentation", "architecture", "plan"]
        let ai_studio_plan = ParsedSearchPlan {
            provider: "gemini".to_string(),
            raw_query: query.to_string(),
            language: "vi".to_string(),
            normalized_query: "tài liệu hướng dẫn".to_string(),
            semantic_keywords: vec![
                "tài liệu".to_string(),
                "hướng dẫn".to_string(),
                "readme".to_string(),
                "guide".to_string(),
                "guidelines".to_string(),
                "specification".to_string(),
                "architecture".to_string(),
                "plan".to_string(),
                "docs".to_string(),
                "documentation".to_string(),
            ],
            name_patterns: vec!["*readme*.*".to_string(), "*guide*.*".to_string()],
            code_routes: vec!["/docs".to_string(), "/guide".to_string()],
            temporal_info: None,
            target_extensions: vec!["md".to_string(), "pdf".to_string()],
            negative_keywords: vec![],
        };

        // Quét các file tài liệu Markdown thực tế ngay trong repository này
        let doc_files = [
            "README.md",
            "AGENTS.md",
            "ARCHITECTURE_PLAN.md",
            "CONVERSATION_CONTEXT.md",
            "PROJECT.md",
        ];

        println!("\n=== KẾT QUẢ TÌM KIẾM CHO KEYWORD: '{}' ===", query);

        println!("\n[1] KẾT QUẢ VỚI WORKFLOW GOOGLE AI STUDIO (Cross-lingual Reasoning):");
        let mut ai_matches = Vec::new();
        for &file_name in &doc_files {
            if let Ok(content) = fs::read_to_string(file_name) {
                let match_result = HybridScorer::score_document(file_name, &content, &ai_studio_plan);
                ai_matches.push(match_result);
            }
        }
        ai_matches.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        for m in &ai_matches {
            println!(
                "  -> File: {:<25} | Điểm: {:>5.1} | Khớp: {:?}",
                m.file_name, m.final_score, m.matched_keywords
            );
        }

        println!("\n[2] KẾT QUẢ VỚI WORKFLOW TIÊU CHUẨN NỘI BỘ (Standard Pure):");
        let mut std_matches = Vec::new();
        for &file_name in &doc_files {
            if let Ok(content) = fs::read_to_string(file_name) {
                let match_result = HybridScorer::score_document(file_name, &content, &standard_plan);
                std_matches.push(match_result);
            }
        }
        std_matches.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        for m in &std_matches {
            println!(
                "  -> File: {:<25} | Điểm: {:>5.1} | Khớp: {:?}",
                m.file_name, m.final_score, m.matched_keywords
            );
        }

        // Kiểm tra tính hiệu quả: Các file hướng dẫn cốt lõi (README.md, AGENTS.md, ARCHITECTURE_PLAN.md) phải lên top
        assert!(ai_matches[0].final_score > 300.0);
        assert!(ai_matches.iter().any(|m| m.file_name == "README.md" || m.file_name == "AGENTS.md"));
    }
}
