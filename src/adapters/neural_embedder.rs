use crate::domain::{DistanceMetric, EmbeddingSpace, EmbeddingVector, SearchError};
use crate::ports::EmbeddingProvider;
use serde::{Deserialize, Serialize};

/// Configuration for neural / model-endpoint embedding providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralModelConfig {
    pub provider_id: String,
    pub model_id: String,
    pub revision: String,
    pub dimensions: usize,
    pub distance_metric: DistanceMetric,
    pub normalized: bool,
    pub supported_languages: Vec<String>,
    pub endpoint_url: Option<String>,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub timeout_ms: u64,
}

impl NeuralModelConfig {
    /// Factory for sentence-transformers/all-multilingual-mpnet-base configuration.
    pub fn all_multilingual_mpnet_base() -> Self {
        Self {
            provider_id: "local-neural-runtime".to_string(),
            model_id: "sentence-transformers/all-multilingual-mpnet-base".to_string(),
            revision: "v1".to_string(),
            dimensions: 768,
            distance_metric: DistanceMetric::Cosine,
            normalized: true,
            supported_languages: vec![
                "vi".to_string(),
                "en".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "es".to_string(),
                "zh".to_string(),
            ],
            endpoint_url: Some("http://127.0.0.1:8080/embed".to_string()),
            api_key: None,
            timeout_ms: 5000,
        }
    }

    /// Factory for BAAI/bge-m3 dense configuration.
    pub fn bge_m3() -> Self {
        Self {
            provider_id: "local-neural-runtime".to_string(),
            model_id: "BAAI/bge-m3".to_string(),
            revision: "v1".to_string(),
            dimensions: 1024,
            distance_metric: DistanceMetric::Cosine,
            normalized: true,
            supported_languages: vec!["vi".to_string(), "en".to_string(), "zh".to_string()],
            endpoint_url: Some("http://127.0.0.1:8080/v1/embeddings".to_string()),
            api_key: None,
            timeout_ms: 5000,
        }
    }

    pub fn to_embedding_space(&self) -> EmbeddingSpace {
        EmbeddingSpace {
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            revision: self.revision.clone(),
            dimensions: self.dimensions,
            distance_metric: self.distance_metric,
            normalized: self.normalized,
            supported_languages: self.supported_languages.clone(),
        }
    }
}

/// Transport interface for dispatching embedding inference requests.
pub trait EmbeddingTransport: Send + Sync {
    fn execute_embedding(
        &self,
        config: &NeuralModelConfig,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, SearchError>;
}

/// Deterministic offline neural mock transport for repeatable local testing and CI.
pub struct DeterministicOfflineNeuralTransport;

impl EmbeddingTransport for DeterministicOfflineNeuralTransport {
    fn execute_embedding(
        &self,
        config: &NeuralModelConfig,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, SearchError> {
        let mut batch_vectors = Vec::with_capacity(texts.len());

        for text in texts {
            let mut vec = vec![0.0f32; config.dimensions];
            let lower = text.to_lowercase();

            // High-dimensional concept subspace projections
            let concepts = [
                // Invoices / Hóa đơn
                (&["hóa đơn", "invoice", "bill", "billing", "dịch vụ", "wire transfer"][..], 0),
                // Receipts / Biên lai
                (&["biên lai", "receipt", "giấy nộp tiền", "thu phí", "lệ phí", "paid"][..], 64),
                // Contracts / Hợp đồng
                (&["hợp đồng", "contract", "agreement", "msa", "thỏa thuận", "signed"][..], 128),
                // Auth / Đăng nhập
                (&["đăng nhập", "login", "auth", "jwt", "credentials", "password"][..], 192),
                // Docs / Tài liệu kiến trúc
                (&["tài liệu", "hướng dẫn", "guidelines", "documentation", "docs", "architecture"][..], 256),
                // Temporal
                (&["tháng 8", "august", "2026", "2608"][..], 320),
            ];

            for (keywords, start_dim) in concepts {
                for kw in keywords {
                    if lower.contains(kw) {
                        for offset in 0..32 {
                            let idx = (start_dim + offset) % config.dimensions;
                            vec[idx] += 1.25 / ((offset % 4 + 1) as f32);
                        }
                    }
                }
            }

            // Continuous hash dispersion for lexical nuances
            for (i, b) in text.as_bytes().iter().enumerate() {
                let dim = (384 + (i * 13) % (config.dimensions - 384)) % config.dimensions;
                vec[dim] += (*b as f32) / 255.0 * 0.15;
            }

            // L2 normalize
            let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
            let norm = norm_sq.sqrt();
            if norm > 0.0 {
                for v in &mut vec {
                    *v /= norm;
                }
            }

            batch_vectors.push(vec);
        }

        Ok(batch_vectors)
    }
}

/// Provider-neutral neural embedding adapter implementing `EmbeddingProvider`.
pub struct ModelEndpointEmbeddingAdapter<T: EmbeddingTransport = DeterministicOfflineNeuralTransport> {
    config: NeuralModelConfig,
    space: EmbeddingSpace,
    transport: T,
}

impl ModelEndpointEmbeddingAdapter<DeterministicOfflineNeuralTransport> {
    pub fn new_offline(config: NeuralModelConfig) -> Self {
        let space = config.to_embedding_space();
        Self {
            config,
            space,
            transport: DeterministicOfflineNeuralTransport,
        }
    }
}

impl<T: EmbeddingTransport> ModelEndpointEmbeddingAdapter<T> {
    pub fn with_transport(config: NeuralModelConfig, transport: T) -> Self {
        let space = config.to_embedding_space();
        Self {
            config,
            space,
            transport,
        }
    }

    pub fn config(&self) -> &NeuralModelConfig {
        &self.config
    }
}

impl<T: EmbeddingTransport> EmbeddingProvider for ModelEndpointEmbeddingAdapter<T> {
    fn embedding_space(&self) -> &EmbeddingSpace {
        &self.space
    }

    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, SearchError> {
        let batch = self.embed_batch(&[text])?;
        batch.into_iter().next().ok_or_else(|| {
            SearchError::Internal("Expected 1 vector from neural embedding adapter".to_string())
        })
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, SearchError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let raw_vectors = self.transport.execute_embedding(&self.config, texts)?;

        if raw_vectors.len() != texts.len() {
            return Err(SearchError::Internal(format!(
                "Neural transport returned {} vectors for {} input texts",
                raw_vectors.len(),
                texts.len()
            )));
        }

        let mut embedding_vectors = Vec::with_capacity(raw_vectors.len());
        for values in raw_vectors {
            if values.len() != self.space.dimensions {
                return Err(SearchError::DimensionMismatch {
                    expected: self.space.dimensions,
                    actual: values.len(),
                });
            }
            embedding_vectors.push(EmbeddingVector {
                space: self.space.clone(),
                values,
            });
        }

        Ok(embedding_vectors)
    }
}
