use crate::domain::{DistanceMetric, EmbeddingSpace, EmbeddingVector, SearchError};
use crate::ports::EmbeddingProvider;

/// A prototype local-first multilingual concept-weighted embedding adapter projecting Vietnamese and English
/// vocabulary cues into a continuous vector space for in-memory prototype testing.
/// 
/// IMPORTANT ARCHITECTURAL NOTE (Finding P1 / Audit 2026-10-12):
/// This embedder serves as an offline deterministic prototype double. It maps known concept signals
/// and byte hashes into vector dimensions. In production environments, replace with an adapter
/// connecting to a trained multilingual embedding model (e.g., multilingual-e5, BGE-M3, or
/// an AI Provider embedding endpoint implementing EmbeddingProvider) to achieve genuine cross-lingual semantics
/// without hand-authored domain vocabulary rules.
pub struct MultilingualSemanticEmbedder {
    space: EmbeddingSpace,
}

impl MultilingualSemanticEmbedder {
    pub const DIMENSIONS: usize = 32;

    pub fn new() -> Self {
        let space = EmbeddingSpace {
            provider_id: "local-semantic-engine".to_string(),
            model_id: "cross-lingual-v1".to_string(),
            revision: "1.0.0".to_string(),
            dimensions: Self::DIMENSIONS,
            distance_metric: DistanceMetric::Cosine,
            normalized: true,
            supported_languages: vec!["vi".to_string(), "en".to_string()],
        };

        Self { space }
    }
}

impl Default for MultilingualSemanticEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingProvider for MultilingualSemanticEmbedder {
    fn embedding_space(&self) -> &EmbeddingSpace {
        &self.space
    }

    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, SearchError> {
        let lower = text.to_lowercase();
        let mut vec = vec![0.0f32; Self::DIMENSIONS];

        // 1. Concept: Financial Invoice / Hóa đơn dịch vụ (Dims 0..3)
        let invoice_signals = [
            "hóa đơn", "hoa don", "invoice", "invoicing", "bill", "billing", "amount due",
            "dịch vụ", "services", "wire transfer", "thuế", "vat",
        ];
        for sig in invoice_signals {
            if lower.contains(sig) {
                vec[0] += 0.85;
                vec[1] += 0.45;
                vec[2] += 0.35;
            }
        }

        // 2. Concept: Payment Receipt / Biên lai nộp tiền lệ phí (Dims 4..7)
        let receipt_signals = [
            "biên lai", "biên lại", "giấy nộp tiền", "giay nop tien", "lệ phí", "thu phí",
            "receipt", "payment slip", "payment receipt", "hộ tịch", "rec-", "received from",
            "paid in full", "thu nộp", "chứng từ",
        ];
        for sig in receipt_signals {
            if lower.contains(sig) {
                vec[4] += 0.90;
                vec[5] += 0.50;
                vec[6] += 0.40;
            }
        }

        // 3. Concept: Legal Contracts / Hợp đồng kinh tế thỏa thuận (Dims 8..11)
        let contract_signals = [
            "hợp đồng", "hop dong", "thỏa thuận", "contract", "agreement", "master services",
            "msa", "signed", "đã ký", "kết ước", "pháp lý", "hai bên", "executed",
        ];
        for sig in contract_signals {
            if lower.contains(sig) {
                vec[8] += 0.85;
                vec[9] += 0.55;
                vec[10] += 0.40;
            }
        }

        // 4. Concept: Authentication / Đăng nhập / Credentials (Dims 12..15)
        let auth_signals = [
            "đăng nhập", "dang nhap", "login", "auth", "authentication", "credentials",
            "jwt", "token", "password", "username", "session", "401", "unauthorized",
        ];
        for sig in auth_signals {
            if lower.contains(sig) {
                vec[12] += 0.90;
                vec[13] += 0.50;
                vec[14] += 0.40;
            }
        }

        // 5. Concept: Documentation & Architecture Guidelines (Dims 16..19)
        let docs_signals = [
            "tài liệu", "hướng dẫn", "kiến trúc", "guidelines", "documentation", "docs",
            "architecture", "manual", "guide", "standards", "specifications", "readme",
        ];
        for sig in docs_signals {
            if lower.contains(sig) {
                vec[16] += 0.85;
                vec[17] += 0.50;
                vec[18] += 0.40;
            }
        }

        // 6. Concept: Temporal August / Month 8 (Dims 20..23)
        let aug_signals = [
            "tháng 8", "tháng 08", "august", "aug", "08/2026", "2608", "260824",
        ];
        for sig in aug_signals {
            if lower.contains(sig) {
                vec[20] += 0.70;
                vec[21] += 0.40;
            }
        }

        // 7. General term hash dispersal across remaining dimensions (Dims 24..31)
        for (i, b) in text.as_bytes().iter().enumerate() {
            let dim = 24 + (i % 8);
            vec[dim] += (*b as f32) / 2550.0;
        }

        // Unit normalize (L2 norm)
        let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
        let norm = norm_sq.sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        EmbeddingVector::new(self.space.clone(), vec)
    }
}
