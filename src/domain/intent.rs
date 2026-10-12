use crate::domain::{QueryFilter, SearchError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryIntentCategory {
    FindDocument,
    DebugCode,
    CompareDocuments,
    GeneralSearch,
}

/// Normalized representation of query intent and constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedQueryIntent {
    pub raw_query: String,
    pub detected_language: Option<String>,
    pub category: QueryIntentCategory,
    /// Mandatory filters that CANNOT be dropped or relaxed by rewriting.
    pub mandatory_filter: QueryFilter,
    /// Bounded soft expansion terms for improving lexical/semantic recall.
    pub expanded_terms: Vec<String>,
    /// Bounded decomposed subqueries for complex multi-condition questions.
    pub subqueries: Vec<String>,
}

impl NormalizedQueryIntent {
    pub const MAX_EXPANDED_TERMS: usize = 20;
    pub const MAX_SUBQUERIES: usize = 5;

    pub fn new(raw_query: impl Into<String>) -> Self {
        let raw = raw_query.into();
        Self {
            raw_query: raw,
            detected_language: None,
            category: QueryIntentCategory::GeneralSearch,
            mandatory_filter: QueryFilter::default(),
            expanded_terms: Vec::new(),
            subqueries: Vec::new(),
        }
    }

    /// Validate invariants: expansion limits and mandatory filter preservation.
    pub fn validate(&self) -> Result<(), SearchError> {
        if self.expanded_terms.len() > Self::MAX_EXPANDED_TERMS {
            return Err(SearchError::FilterViolation(format!(
                "Expanded terms count {} exceeds limit {}",
                self.expanded_terms.len(),
                Self::MAX_EXPANDED_TERMS
            )));
        }

        if self.subqueries.len() > Self::MAX_SUBQUERIES {
            return Err(SearchError::FilterViolation(format!(
                "Subqueries count {} exceeds limit {}",
                self.subqueries.len(),
                Self::MAX_SUBQUERIES
            )));
        }

        // Check that any negative keyword in raw query has not been dropped
        let lower = self.raw_query.to_lowercase();
        let negation_cues = ["không phải", "loại trừ", "không lấy", "not ", "exclude "];
        for cue in negation_cues {
            if lower.contains(cue) && self.mandatory_filter.negative_keywords.is_empty() {
                return Err(SearchError::FilterViolation(format!(
                    "Query contained negation cue '{}' but mandatory negative keywords filter was empty",
                    cue
                )));
            }
        }

        Ok(())
    }
}
