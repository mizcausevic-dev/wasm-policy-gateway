//! Pure policy logic for an edge gateway: geo, rate limiting, and A/B
//! routing. No I/O — every function is deterministic given its inputs,
//! so it's trivially testable natively and equally happy compiled to
//! WASI.
//!
//! Decision pipeline:
//! 1. **Geo gate** — block by country code.
//! 2. **Rate limit** — token bucket per `rate_bucket` value.
//! 3. **A/B routing** — deterministic redirect for a configured fraction.
//!
//! First gate to fire decides the outcome. The engine carries no
//! mutable state across calls — rate limit state is part of the request.

use serde::{Deserialize, Serialize};

/// The input the gateway evaluates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInput {
    /// Client country code (ISO 3166-1 alpha-2). Optional.
    #[serde(default)]
    pub country: Option<String>,
    /// Identifier of the bucket the request is rate-limited against.
    #[serde(default)]
    pub rate_bucket: Option<String>,
    /// Tokens currently remaining in the bucket prior to this request.
    #[serde(default)]
    pub rate_tokens_remaining: i64,
    /// A/B bucket the request belongs to (stable hash of user id).
    #[serde(default)]
    pub ab_bucket: Option<String>,
    /// Original URL (for redirect decisions).
    #[serde(default)]
    pub original_url: Option<String>,
}

/// The decision returned by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyDecision {
    pub decision: Decision,
    pub policy_id: String,
    pub redirect_to: Option<String>,
    pub rate_tokens_after: Option<i64>,
    pub reasons: Vec<String>,
}

/// Possible outcomes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    Redirect,
}

/// The configuration the engine applies. In production these are
/// loaded from a control plane; for a single-call evaluation they are
/// passed in alongside the input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub policy_id: String,
    /// Country codes that are denied outright.
    #[serde(default)]
    pub blocked_countries: Vec<String>,
    /// Buckets that should be A/B-redirected, and where to.
    #[serde(default)]
    pub ab_redirects: Vec<AbRule>,
}

/// One A/B redirect rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbRule {
    pub ab_bucket: String,
    pub redirect_to: String,
}

impl PolicyConfig {
    /// Evaluate the configured policy against an input.
    pub fn evaluate(&self, input: &PolicyInput) -> PolicyDecision {
        // Pipeline: geo → rate → ab → allow.
        if let Some(decision) = self.evaluate_geo(input) {
            return decision;
        }
        if let Some(decision) = self.evaluate_rate(input) {
            return decision;
        }
        if let Some(decision) = self.evaluate_ab(input) {
            return decision;
        }
        PolicyDecision {
            decision: Decision::Allow,
            policy_id: self.policy_id.clone(),
            redirect_to: None,
            rate_tokens_after: Some(input.rate_tokens_remaining - 1),
            reasons: vec!["allow_default".into()],
        }
    }

    fn evaluate_geo(&self, input: &PolicyInput) -> Option<PolicyDecision> {
        if let Some(country) = input.country.as_deref() {
            let blocked = self
                .blocked_countries
                .iter()
                .any(|c| c.eq_ignore_ascii_case(country));
            if blocked {
                return Some(PolicyDecision {
                    decision: Decision::Deny,
                    policy_id: self.policy_id.clone(),
                    redirect_to: None,
                    rate_tokens_after: None,
                    reasons: vec![format!("geo_blocked:{}", country.to_uppercase())],
                });
            }
        }
        None
    }

    fn evaluate_rate(&self, input: &PolicyInput) -> Option<PolicyDecision> {
        if input.rate_bucket.is_some() && input.rate_tokens_remaining <= 0 {
            return Some(PolicyDecision {
                decision: Decision::Deny,
                policy_id: self.policy_id.clone(),
                redirect_to: None,
                rate_tokens_after: Some(input.rate_tokens_remaining),
                reasons: vec!["rate_limit_exhausted".into()],
            });
        }
        None
    }

    fn evaluate_ab(&self, input: &PolicyInput) -> Option<PolicyDecision> {
        if let Some(bucket) = input.ab_bucket.as_deref() {
            for rule in &self.ab_redirects {
                if rule.ab_bucket.eq_ignore_ascii_case(bucket) {
                    return Some(PolicyDecision {
                        decision: Decision::Redirect,
                        policy_id: self.policy_id.clone(),
                        redirect_to: Some(rule.redirect_to.clone()),
                        rate_tokens_after: Some(input.rate_tokens_remaining - 1),
                        reasons: vec![format!("ab_redirect:{}", bucket)],
                    });
                }
            }
        }
        None
    }
}

/// Combined request envelope — both config and input. This is the
/// shape the WASI binary reads from stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRequest {
    pub config: PolicyConfig,
    pub input: PolicyInput,
}

/// One-shot evaluation that parses a JSON request, evaluates, and
/// returns the JSON decision.
pub fn evaluate_json(raw: &str) -> Result<String, serde_json::Error> {
    let request: EvaluationRequest = serde_json::from_str(raw)?;
    let decision = request.config.evaluate(&request.input);
    serde_json::to_string(&decision)
}
