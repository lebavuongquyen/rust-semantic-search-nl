use crate::domain::{SearchResult, SearchError};
use std::collections::HashSet;

/// Policy controlling bounded execution for multi-step agentic search.
#[derive(Debug, Clone)]
pub struct AgentExecutionPolicy {
    pub max_iterations: usize,
    pub max_subqueries: usize,
}

impl Default for AgentExecutionPolicy {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            max_subqueries: 4,
        }
    }
}

/// Controlled search agent that safely decomposes complex multi-condition queries
/// with strict iteration bounds and loop detection.
pub struct ControlledSearchAgent {
    pub policy: AgentExecutionPolicy,
}

impl ControlledSearchAgent {
    pub fn new(policy: AgentExecutionPolicy) -> Self {
        Self { policy }
    }

    /// Decompose a complex natural-language query into bounded subqueries.
    pub fn decompose_query(&self, raw_query: &str) -> Vec<String> {
        let clean = raw_query.trim();
        let lower = clean.to_lowercase();

        // Check for conjunction delimiters: "và", "and", ";", "cùng với"
        let conjunctions = [" và ", " and ", " ; ", ";", " cùng với "];
        let mut subqueries = Vec::new();

        for conj in conjunctions {
            if lower.contains(conj) {
                for part in clean.split(conj) {
                    let part_clean = part.trim();
                    if !part_clean.is_empty() && !subqueries.contains(&part_clean.to_string()) {
                        subqueries.push(part_clean.to_string());
                    }
                }
                break;
            }
        }

        if subqueries.is_empty() {
            subqueries.push(clean.to_string());
        }

        if subqueries.len() > self.policy.max_subqueries {
            subqueries.truncate(self.policy.max_subqueries);
        }

        subqueries
    }

    /// Execute multi-step search across decomposed subqueries with loop detection and result deduplication.
    pub fn execute_bounded_search<F>(
        &self,
        raw_query: &str,
        mut search_fn: F,
    ) -> Result<Vec<SearchResult>, SearchError>
    where
        F: FnMut(&str) -> Result<Vec<SearchResult>, SearchError>,
    {
        let subqueries = self.decompose_query(raw_query);
        let mut merged_results: Vec<SearchResult> = Vec::new();
        let mut seen_doc_ids: HashSet<String> = HashSet::new();
        let mut seen_iterations_hashes: HashSet<u64> = HashSet::new();

        let mut iterations = 0;

        for subq in &subqueries {
            if iterations >= self.policy.max_iterations {
                break;
            }
            iterations += 1;

            let sub_results = search_fn(subq)?;

            // Loop detection: compute hash of returned document IDs in this iteration
            let mut iter_hash: u64 = 0;
            for r in &sub_results {
                for b in r.document_id.0.as_bytes() {
                    iter_hash = iter_hash.wrapping_mul(31).wrapping_add(*b as u64);
                }
            }

            if !sub_results.is_empty() && !seen_iterations_hashes.insert(iter_hash) {
                // Loop detected: identical result set returned from a different subquery; terminate early
                break;
            }

            // Deduplicate and aggregate results, preserving provenance
            for mut item in sub_results {
                if seen_doc_ids.insert(item.document_id.0.clone()) {
                    let mut trace = item.provenance.trace_details.clone();
                    trace.insert("subquery_origin".to_string(), subq.clone());
                    trace.insert("agent_iteration".to_string(), iterations.to_string());
                    item.provenance.trace_details = trace;
                    merged_results.push(item);
                }
            }
        }

        // Re-assign unified ranks
        for (i, r) in merged_results.iter_mut().enumerate() {
            r.rank = i + 1;
        }

        Ok(merged_results)
    }
}
