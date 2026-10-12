use crate::domain::SearchResult;
use regex::Regex;

/// Multi-tenant and role-based access control policy.
#[derive(Debug, Clone)]
pub struct AccessControlPolicy {
    pub tenant_id: String,
    pub user_roles: Vec<String>,
}

impl AccessControlPolicy {
    pub fn new(tenant_id: impl Into<String>, user_roles: Vec<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_roles,
        }
    }

    /// Filter search results, enforcing strict tenant isolation (fail-closed) and role permissions.
    pub fn filter_authorized_results(&self, candidates: Vec<SearchResult>) -> Vec<SearchResult> {
        candidates
            .into_iter()
            .filter(|res| {
                // Fail closed: result MUST have tenant_id matching active tenant
                let doc_tenant = match res.provenance.trace_details.get("tenant_id") {
                    Some(t) => t,
                    None => return false, // Reject documents with missing tenant identity
                };

                if doc_tenant != &self.tenant_id {
                    return false;
                }

                // If document specifies required_roles, check if user possesses at least one
                if let Some(roles_str) = res.provenance.trace_details.get("required_roles") {
                    let required: Vec<&str> = roles_str.split(',').map(|r| r.trim()).collect();
                    let has_role = required.iter().any(|req| {
                        self.user_roles.iter().any(|user_role| user_role == req)
                    });
                    if !has_role {
                        return false;
                    }
                }

                true
            })
            .collect()
    }
}

/// Secret and PII redaction policy for privacy-compliant observability.
pub struct TelemetryRedactionPolicy;

impl TelemetryRedactionPolicy {
    /// Redact API keys, bearer tokens, and secrets from diagnostic strings.
    pub fn redact(input: &str) -> String {
        // Redact Google AI Studio / Gemini keys: AIzaSy...
        let re_google_key = Regex::new(r"AIza[0-9A-Za-z-_]{35}").unwrap();
        let s1 = re_google_key.replace_all(input, "[REDACTED_API_KEY]");

        // Redact Bearer tokens
        let re_bearer = Regex::new(r"(?i)bearer\s+[A-Za-z0-9\-\._~\+\/]+=*").unwrap();
        let s2 = re_bearer.replace_all(&s1, "Bearer [REDACTED_TOKEN]");

        // Redact generic API keys: key=... or apiKey=...
        let re_query_key = Regex::new(r"(?i)(key|apiKey)=[A-Za-z0-9_\-]+").unwrap();
        let s3 = re_query_key.replace_all(&s2, "$1=[REDACTED]");

        s3.to_string()
    }
}
