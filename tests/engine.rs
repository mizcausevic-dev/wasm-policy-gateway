use policy_gateway::{
    evaluate_json, AbRule, Decision, EvaluationRequest, PolicyConfig, PolicyInput,
};
use pretty_assertions::assert_eq;

fn default_config() -> PolicyConfig {
    PolicyConfig {
        policy_id: "test-2026-05".into(),
        blocked_countries: vec!["KP".into(), "IR".into()],
        ab_redirects: vec![AbRule {
            ab_bucket: "experimental".into(),
            redirect_to: "https://canary.example.com".into(),
        }],
    }
}

#[test]
fn allow_for_safe_country_with_budget() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("US".into()),
        rate_bucket: Some("free".into()),
        rate_tokens_remaining: 100,
        ab_bucket: None,
        original_url: Some("https://example.com/x".into()),
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Allow);
    assert_eq!(decision.rate_tokens_after, Some(99));
}

#[test]
fn deny_for_blocked_country() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("KP".into()),
        rate_bucket: Some("free".into()),
        rate_tokens_remaining: 100,
        ab_bucket: None,
        original_url: None,
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Deny);
    assert!(decision.reasons[0].starts_with("geo_blocked:KP"));
}

#[test]
fn deny_for_blocked_country_case_insensitive() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("kp".into()), // lowercase
        rate_bucket: None,
        rate_tokens_remaining: 0,
        ab_bucket: None,
        original_url: None,
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Deny);
}

#[test]
fn deny_when_rate_limit_exhausted() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("US".into()),
        rate_bucket: Some("free".into()),
        rate_tokens_remaining: 0,
        ab_bucket: None,
        original_url: None,
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Deny);
    assert_eq!(decision.reasons, vec!["rate_limit_exhausted".to_string()]);
}

#[test]
fn redirect_for_ab_bucket() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("US".into()),
        rate_bucket: Some("paid".into()),
        rate_tokens_remaining: 50,
        ab_bucket: Some("experimental".into()),
        original_url: Some("https://example.com/".into()),
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Redirect);
    assert_eq!(
        decision.redirect_to,
        Some("https://canary.example.com".to_string())
    );
}

#[test]
fn geo_block_outranks_rate_and_ab() {
    let cfg = default_config();
    let input = PolicyInput {
        country: Some("IR".into()),
        rate_bucket: Some("free".into()),
        rate_tokens_remaining: 0,               // would also rate-limit
        ab_bucket: Some("experimental".into()), // would also AB-redirect
        original_url: None,
    };
    let decision = cfg.evaluate(&input);
    assert_eq!(decision.decision, Decision::Deny);
    assert!(decision.reasons[0].starts_with("geo_blocked"));
}

#[test]
fn evaluate_json_round_trip() {
    let raw = serde_json::to_string(&EvaluationRequest {
        config: default_config(),
        input: PolicyInput {
            country: Some("US".into()),
            rate_bucket: Some("paid".into()),
            rate_tokens_remaining: 10,
            ab_bucket: None,
            original_url: None,
        },
    })
    .unwrap();
    let response = evaluate_json(&raw).unwrap();
    let decision: policy_gateway::PolicyDecision = serde_json::from_str(&response).unwrap();
    assert_eq!(decision.decision, Decision::Allow);
}
