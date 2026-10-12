use crate::domain::{ScoreComponent, SearchError, SearchResult};
use crate::ports::RerankerPort;

/// Lexical heuristic candidate reranker optimizing ordering and precision.
/// Note: This is a fast lexical heuristic reranker using title matching and token overlap density;
/// for deep neural cross-encoding, connect an external model through RerankerPort.
pub struct LexicalHeuristicReranker {
    pub max_candidates: usize,
    pub negative_penalty_terms: Vec<String>,
}

/// Backwards-compatible type alias
pub type CrossEncoderReranker = LexicalHeuristicReranker;

impl Default for LexicalHeuristicReranker {
    fn default() -> Self {
        Self::new(20)
    }
}

impl LexicalHeuristicReranker {
    pub fn new(max_candidates: usize) -> Self {
        Self {
            max_candidates,
            negative_penalty_terms: vec!["draft".to_string(), "unapproved".to_string()],
        }
    }

    pub fn with_penalty_terms(mut self, terms: Vec<String>) -> Self {
        self.negative_penalty_terms = terms;
        self
    }
}

impl RerankerPort for LexicalHeuristicReranker {
    fn rerank(
        &self,
        query: &str,
        mut candidates: Vec<SearchResult>,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, SearchError> {
        if candidates.is_empty() {
            return Ok(candidates);
        }

        let query_lower = query.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        // Limit candidates to max_candidates budget
        if candidates.len() > self.max_candidates {
            candidates.truncate(self.max_candidates);
        }

        for candidate in &mut candidates {
            let title_lower = candidate.document_title.to_lowercase();
            let snippet_lower = candidate.snippet.to_lowercase();

            let mut boost = 0.0f32;

            // 1. Exact query match in title
            if title_lower.contains(&query_lower) {
                boost += 0.50;
            }

            // 2. Token overlap density
            let mut matched_token_count = 0;
            for token in &query_tokens {
                if title_lower.contains(token) || snippet_lower.contains(token) {
                    matched_token_count += 1;
                }
            }
            let overlap_ratio = (matched_token_count as f32) / (query_tokens.len().max(1) as f32);
            boost += overlap_ratio * 0.35;

            // 3. Negative penalty terms (hard negative mitigation)
            let has_penalty_term = self.negative_penalty_terms.iter().any(|term| {
                let term_lower = term.to_lowercase();
                title_lower.contains(&term_lower) || snippet_lower.contains(&term_lower)
            });
            if has_penalty_term {
                boost -= 0.60;
            }

            // Append score component
            candidate.score.final_score += boost;
            candidate.score.components.push(ScoreComponent {
                name: "cross_encoder_rerank".to_string(),
                raw_value: boost,
                weighted_value: boost,
                description: format!("Reranker score adjustment: {:+.4}", boost),
            });
        }

        // Re-sort descending by final score
        candidates.sort_by(|a, b| {
            b.score
                .final_score
                .partial_cmp(&a.score.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Re-assign ranks 1..=top_k
        for (i, c) in candidates.iter_mut().enumerate() {
            c.rank = i + 1;
        }

        if candidates.len() > top_k {
            candidates.truncate(top_k);
        }

        Ok(candidates)
    }
}
