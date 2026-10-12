use crate::domain::{Document, SearchResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An annotated golden query for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenQuery {
    pub id: String,
    pub query: String,
    pub query_language: String,
    pub expected_document_language: String,
    pub description: String,
    /// Map of document_id -> graded relevance score (e.g. 2 for highly relevant, 1 for relevant)
    pub relevant_docs: HashMap<String, u32>,
    #[serde(default)]
    pub hard_negatives: Vec<String>,
}

/// Evaluation metrics for a single query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEvaluationMetric {
    pub query_id: String,
    pub query_text: String,
    pub recall_at_k: f32,
    pub precision_at_k: f32,
    pub reciprocal_rank: f32,
    pub ndcg_at_k: f32,
    pub retrieved_doc_ids: Vec<String>,
    #[serde(default)]
    pub duration_ms: f64,
}

/// Sliced metric aggregation (e.g. by language, category, or difficulty).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MetricSlice {
    pub slice_name: String,
    pub count: usize,
    pub mean_recall: f32,
    pub mean_precision: f32,
    pub mean_mrr: f32,
    pub mean_ndcg: f32,
    pub mean_duration_ms: f64,
}

/// Overall aggregated evaluation summary report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub configuration_name: String,
    pub total_queries: usize,
    pub k: usize,
    pub mean_recall: f32,
    pub mean_precision: f32,
    pub mean_reciprocal_rank: f32,
    pub mean_ndcg: f32,
    #[serde(default)]
    pub mean_duration_ms: f64,
    #[serde(default)]
    pub p50_duration_ms: f64,
    #[serde(default)]
    pub p95_duration_ms: f64,
    #[serde(default)]
    pub slices: HashMap<String, MetricSlice>,
    pub query_metrics: Vec<QueryEvaluationMetric>,
}

/// Regression gate configuration thresholds for continuous quality checks (M09).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionGateConfig {
    pub max_recall_drop: f32,
    pub max_mrr_drop: f32,
    pub max_ndcg_drop: f32,
    pub max_latency_increase_ratio: f64,
    pub max_p95_duration_ms: Option<f64>,
    pub min_canary_recall: Option<f32>,
}

impl Default for RegressionGateConfig {
    fn default() -> Self {
        Self {
            max_recall_drop: 0.05,
            max_mrr_drop: 0.05,
            max_ndcg_drop: 0.05,
            max_latency_increase_ratio: 0.50, // 50% increase max
            max_p95_duration_ms: Some(1000.0),
            min_canary_recall: Some(0.80),
        }
    }
}

/// Specific violation reason when a candidate strategy regresses against baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegressionViolation {
    RecallDrop {
        baseline: f32,
        candidate: f32,
        drop: f32,
        max_allowed: f32,
    },
    MrrDrop {
        baseline: f32,
        candidate: f32,
        drop: f32,
        max_allowed: f32,
    },
    NdcgDrop {
        baseline: f32,
        candidate: f32,
        drop: f32,
        max_allowed: f32,
    },
    LatencySpike {
        baseline_ms: f64,
        candidate_ms: f64,
        increase_ratio: f64,
        max_allowed_ratio: f64,
    },
    P95LatencyExceeded {
        candidate_p95_ms: f64,
        max_allowed_ms: f64,
    },
    CanaryRecallTooLow {
        candidate_recall: f32,
        min_allowed: f32,
    },
}

/// Outcome of continuous evaluation regression check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegressionGateOutcome {
    Passed,
    Rejected(Vec<RegressionViolation>),
}

impl RegressionGateOutcome {
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

pub struct EvaluationHarness;

impl EvaluationHarness {
    pub fn parse_golden_corpus(json_str: &str) -> Result<Vec<Document>, serde_json::Error> {
        #[derive(Deserialize)]
        struct RawDoc {
            id: String,
            title: String,
            file_path: Option<String>,
            content: String,
            #[serde(default)]
            metadata: HashMap<String, String>,
        }

        let raw_docs: Vec<RawDoc> = serde_json::from_str(json_str)?;
        let docs = raw_docs
            .into_iter()
            .map(|d| {
                let mut doc = Document::new(d.id, d.title, d.content);
                doc.file_path = d.file_path;
                doc.metadata = d.metadata;
                doc
            })
            .collect();

        Ok(docs)
    }

    pub fn parse_golden_queries(json_str: &str) -> Result<Vec<GoldenQuery>, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Evaluate search results for a single query against relevance judgments.
    pub fn evaluate_query_results(
        golden: &GoldenQuery,
        results: &[SearchResult],
        k: usize,
    ) -> QueryEvaluationMetric {
        Self::evaluate_query_results_with_duration(golden, results, k, 0.0)
    }

    /// Evaluate search results with explicit duration measurement.
    pub fn evaluate_query_results_with_duration(
        golden: &GoldenQuery,
        results: &[SearchResult],
        k: usize,
        duration_ms: f64,
    ) -> QueryEvaluationMetric {
        let top_results: Vec<&SearchResult> = results.iter().take(k).collect();
        let total_relevant = golden.relevant_docs.values().filter(|&&v| v > 0).count();

        let mut relevant_retrieved_count = 0;
        let mut reciprocal_rank = 0.0f32;
        let mut dcg = 0.0f32;
        let mut retrieved_doc_ids = Vec::new();

        for (rank_0, res) in top_results.iter().enumerate() {
            let doc_id = &res.document_id.0;
            retrieved_doc_ids.push(doc_id.clone());

            let rel = golden.relevant_docs.get(doc_id).copied().unwrap_or(0);
            if rel > 0 {
                relevant_retrieved_count += 1;
                if reciprocal_rank == 0.0 {
                    reciprocal_rank = 1.0 / ((rank_0 + 1) as f32);
                }
            }

            // DCG formula: (2^rel - 1) / log2(rank + 1)
            let gain = (2u32.pow(rel) - 1) as f32;
            let discount = ((rank_0 + 2) as f32).log2();
            dcg += gain / discount;
        }

        // Ideal DCG (IDCG)
        let mut ideal_rels: Vec<u32> = golden.relevant_docs.values().copied().collect();
        ideal_rels.sort_by(|a, b| b.cmp(a));
        let mut idcg = 0.0f32;
        for (rank_0, &rel) in ideal_rels.iter().take(k).enumerate() {
            let gain = (2u32.pow(rel) - 1) as f32;
            let discount = ((rank_0 + 2) as f32).log2();
            idcg += gain / discount;
        }

        let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };
        let recall = if total_relevant > 0 {
            (relevant_retrieved_count as f32) / (total_relevant as f32)
        } else {
            0.0
        };
        let precision = if !top_results.is_empty() {
            (relevant_retrieved_count as f32) / (top_results.len() as f32)
        } else {
            0.0
        };

        QueryEvaluationMetric {
            query_id: golden.id.clone(),
            query_text: golden.query.clone(),
            recall_at_k: recall,
            precision_at_k: precision,
            reciprocal_rank,
            ndcg_at_k: ndcg,
            retrieved_doc_ids,
            duration_ms,
        }
    }

    /// Aggregate query metrics into an overall evaluation report.
    pub fn generate_report(
        config_name: impl Into<String>,
        k: usize,
        metrics: Vec<QueryEvaluationMetric>,
    ) -> EvaluationReport {
        Self::generate_report_with_slices(config_name, k, metrics, HashMap::new())
    }

    /// Aggregate query metrics and include sliced dimensions.
    pub fn generate_report_with_slices(
        config_name: impl Into<String>,
        k: usize,
        metrics: Vec<QueryEvaluationMetric>,
        slices: HashMap<String, MetricSlice>,
    ) -> EvaluationReport {
        let n = metrics.len().max(1) as f32;
        let mean_recall = metrics.iter().map(|m| m.recall_at_k).sum::<f32>() / n;
        let mean_precision = metrics.iter().map(|m| m.precision_at_k).sum::<f32>() / n;
        let mean_mrr = metrics.iter().map(|m| m.reciprocal_rank).sum::<f32>() / n;
        let mean_ndcg = metrics.iter().map(|m| m.ndcg_at_k).sum::<f32>() / n;

        let mean_duration = metrics.iter().map(|m| m.duration_ms).sum::<f64>()
            / (metrics.len().max(1) as f64);

        let mut durations: Vec<f64> = metrics.iter().map(|m| m.duration_ms).collect();
        durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let (p50, p95) = if durations.is_empty() {
            (0.0, 0.0)
        } else {
            let p50_idx = (durations.len() * 50) / 100;
            let p95_idx = ((durations.len() * 95) / 100).min(durations.len() - 1);
            (durations[p50_idx], durations[p95_idx])
        };

        EvaluationReport {
            configuration_name: config_name.into(),
            total_queries: metrics.len(),
            k,
            mean_recall,
            mean_precision,
            mean_reciprocal_rank: mean_mrr,
            mean_ndcg,
            mean_duration_ms: mean_duration,
            p50_duration_ms: p50,
            p95_duration_ms: p95,
            slices,
            query_metrics: metrics,
        }
    }

    /// Compute sliced performance breakdowns from golden queries and query metrics.
    pub fn compute_slices(
        golden_queries: &[GoldenQuery],
        metrics: &[QueryEvaluationMetric],
    ) -> HashMap<String, MetricSlice> {
        let mut slice_groups: HashMap<String, Vec<&QueryEvaluationMetric>> = HashMap::new();
        let query_map: HashMap<&str, &GoldenQuery> = golden_queries
            .iter()
            .map(|q| (q.id.as_str(), q))
            .collect();

        for m in metrics {
            if let Some(golden) = query_map.get(m.query_id.as_str()) {
                let lang_key = format!("lang:{}", golden.query_language);
                slice_groups.entry(lang_key).or_default().push(m);

                let target_lang_key = format!("target_lang:{}", golden.expected_document_language);
                slice_groups.entry(target_lang_key).or_default().push(m);

                let cross_key = if golden.query_language != golden.expected_document_language {
                    "slice:cross_lingual".to_string()
                } else {
                    "slice:mono_lingual".to_string()
                };
                slice_groups.entry(cross_key).or_default().push(m);
            }
        }

        let mut slices = HashMap::new();
        for (name, group) in slice_groups {
            let count = group.len();
            if count == 0 {
                continue;
            }
            let c_f = count as f32;
            let mean_recall = group.iter().map(|g| g.recall_at_k).sum::<f32>() / c_f;
            let mean_precision = group.iter().map(|g| g.precision_at_k).sum::<f32>() / c_f;
            let mean_mrr = group.iter().map(|g| g.reciprocal_rank).sum::<f32>() / c_f;
            let mean_ndcg = group.iter().map(|g| g.ndcg_at_k).sum::<f32>() / c_f;
            let mean_dur = group.iter().map(|g| g.duration_ms).sum::<f64>() / (count as f64);

            slices.insert(
                name.clone(),
                MetricSlice {
                    slice_name: name,
                    count,
                    mean_recall,
                    mean_precision,
                    mean_mrr,
                    mean_ndcg,
                    mean_duration_ms: mean_dur,
                },
            );
        }

        slices
    }

    /// Check if a candidate strategy regresses against an established baseline.
    pub fn evaluate_regression(
        baseline: &EvaluationReport,
        candidate: &EvaluationReport,
        config: &RegressionGateConfig,
    ) -> RegressionGateOutcome {
        let mut violations = Vec::new();

        // Check recall drop
        let recall_drop = baseline.mean_recall - candidate.mean_recall;
        if recall_drop > config.max_recall_drop {
            violations.push(RegressionViolation::RecallDrop {
                baseline: baseline.mean_recall,
                candidate: candidate.mean_recall,
                drop: recall_drop,
                max_allowed: config.max_recall_drop,
            });
        }

        // Check MRR drop
        let mrr_drop = baseline.mean_reciprocal_rank - candidate.mean_reciprocal_rank;
        if mrr_drop > config.max_mrr_drop {
            violations.push(RegressionViolation::MrrDrop {
                baseline: baseline.mean_reciprocal_rank,
                candidate: candidate.mean_reciprocal_rank,
                drop: mrr_drop,
                max_allowed: config.max_mrr_drop,
            });
        }

        // Check NDCG drop
        let ndcg_drop = baseline.mean_ndcg - candidate.mean_ndcg;
        if ndcg_drop > config.max_ndcg_drop {
            violations.push(RegressionViolation::NdcgDrop {
                baseline: baseline.mean_ndcg,
                candidate: candidate.mean_ndcg,
                drop: ndcg_drop,
                max_allowed: config.max_ndcg_drop,
            });
        }

        // Check latency increase ratio
        if baseline.mean_duration_ms > 0.0 {
            let latency_ratio = (candidate.mean_duration_ms - baseline.mean_duration_ms)
                / baseline.mean_duration_ms;
            if latency_ratio > config.max_latency_increase_ratio {
                violations.push(RegressionViolation::LatencySpike {
                    baseline_ms: baseline.mean_duration_ms,
                    candidate_ms: candidate.mean_duration_ms,
                    increase_ratio: latency_ratio,
                    max_allowed_ratio: config.max_latency_increase_ratio,
                });
            }
        }

        // Check p95 absolute latency cap
        if let Some(max_p95) = config.max_p95_duration_ms {
            if candidate.p95_duration_ms > max_p95 {
                violations.push(RegressionViolation::P95LatencyExceeded {
                    candidate_p95_ms: candidate.p95_duration_ms,
                    max_allowed_ms: max_p95,
                });
            }
        }

        // Check canary minimum recall
        if let Some(min_canary) = config.min_canary_recall {
            if candidate.mean_recall < min_canary {
                violations.push(RegressionViolation::CanaryRecallTooLow {
                    candidate_recall: candidate.mean_recall,
                    min_allowed: min_canary,
                });
            }
        }

        if violations.is_empty() {
            RegressionGateOutcome::Passed
        } else {
            RegressionGateOutcome::Rejected(violations)
        }
    }
}
