//! The reviewer flow (ITEM-12, Codex `auto_review`) — a cheap model risk-
//! classifies a tool call that already needs approval, mapping the risk onto a
//! `Decision`. **Fail-closed**: any classifier error → `Deny` (Codex).
//!
//! The loop reaches this only for a `Decision::Review` outcome (the approval
//! matrix, `crate::policy`): `Low → Auto` (proceed), `High → Prompt` (escalate
//! to the durable `HumanGate`), `Critical → Deny`.

use std::collections::HashMap;
use std::sync::Arc;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock};
use async_trait::async_trait;
use ziee_core::AppError;

use crate::core::ModelClient;
use crate::types::{Decision, ToolCall};

/// The risk classes a tool call is sorted into (Codex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Risk {
    Low,
    High,
    Critical,
}

/// The DEFAULT risk ladder (Codex mapping): `Low → Auto`, `High → Prompt`,
/// `Critical → Deny`. Used verbatim for any band an admin threshold map omits.
pub fn map_risk(risk: Risk) -> Decision {
    match risk {
        Risk::Low => Decision::Auto,
        Risk::High => Decision::Prompt,
        Risk::Critical => Decision::Deny,
    }
}

/// Admin-supplied per-band → decision overrides for the reviewer (ITEM-38 /
/// DEC-83/84). Parsed from a JSON object like `{"high":"deny"}`; any band the
/// map OMITS falls back to the default ladder ([`map_risk`]).
///
/// **Domain-free:** the crate only ever receives already-parsed data — the
/// server reads the `agent_admin_settings.reviewer_risk_thresholds` jsonb and
/// hands it in via [`RiskThresholds::from_json`] + [`Reviewer::new_with_thresholds`].
/// No DB access here. This fixes the live dead-config bug where the admin's
/// stored + validated map was never consulted (`map_risk` was hardcoded).
#[derive(Debug, Clone, Default)]
pub struct RiskThresholds {
    overrides: HashMap<Risk, Decision>,
}

impl RiskThresholds {
    /// Parse from a JSON object of `{"<band>": "<decision>"}` (case-insensitive
    /// on both keys and values). Unknown bands / decisions are ignored (that
    /// band keeps the default ladder); a non-object value yields NO overrides
    /// (pure default ladder). Never errors — a malformed admin value degrades to
    /// the safe default rather than failing the reviewer.
    pub fn from_json(value: &serde_json::Value) -> Self {
        let mut overrides = HashMap::new();
        if let Some(obj) = value.as_object() {
            for (band, decision) in obj {
                if let (Some(risk), Some(dec)) = (
                    parse_band(band),
                    decision.as_str().and_then(parse_decision),
                ) {
                    overrides.insert(risk, dec);
                }
            }
        }
        Self { overrides }
    }

    /// Resolve a risk band to a decision: the admin override when present, else
    /// the default ladder ([`map_risk`]).
    pub fn resolve(&self, risk: Risk) -> Decision {
        self.overrides
            .get(&risk)
            .copied()
            .unwrap_or_else(|| map_risk(risk))
    }

    /// True when no band overrides are set (pure default ladder).
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

fn parse_band(band: &str) -> Option<Risk> {
    match band.trim().to_ascii_lowercase().as_str() {
        "low" => Some(Risk::Low),
        "high" => Some(Risk::High),
        "critical" => Some(Risk::Critical),
        _ => None,
    }
}

fn parse_decision(decision: &str) -> Option<Decision> {
    match decision.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(Decision::Auto),
        "prompt" => Some(Decision::Prompt),
        "review" => Some(Decision::Review),
        "deny" => Some(Decision::Deny),
        _ => None,
    }
}

/// Classify the risk of a tool call under a policy. The seam that makes the
/// reviewer testable without a real model (a fake classifier in tests).
#[async_trait]
pub trait RiskClassifier: Send + Sync {
    async fn classify(&self, call: &ToolCall, policy: &str) -> Result<Risk, AppError>;
}

/// The reviewer: classify → map, fail-closed on any error.
#[derive(Clone)]
pub struct Reviewer {
    pub classifier: Arc<dyn RiskClassifier>,
    /// Admin-steerable reviewer policy text passed to the classifier.
    pub policy: String,
    /// Admin per-band → decision overrides (ITEM-38 / DEC-83). Empty → the
    /// default ladder ([`map_risk`]).
    pub thresholds: RiskThresholds,
}

impl Reviewer {
    /// Construct with the DEFAULT risk ladder (no admin overrides) — preserves
    /// the historical `Low→Auto / High→Prompt / Critical→Deny` behavior. Use
    /// [`Reviewer::new_with_thresholds`] to thread the admin-configured map.
    pub fn new(classifier: Arc<dyn RiskClassifier>, policy: impl Into<String>) -> Self {
        Self::new_with_thresholds(classifier, policy, RiskThresholds::default())
    }

    /// Construct with admin-supplied per-band → decision overrides (DEC-83). The
    /// server passes `RiskThresholds::from_json(&settings.reviewer_risk_thresholds)`.
    pub fn new_with_thresholds(
        classifier: Arc<dyn RiskClassifier>,
        policy: impl Into<String>,
        thresholds: RiskThresholds,
    ) -> Self {
        Self {
            classifier,
            policy: policy.into(),
            thresholds,
        }
    }

    /// Resolve a `Decision::Review` into a concrete `Decision`, mapping the
    /// classified risk through the admin thresholds (default ladder for any
    /// omitted band). FAIL-CLOSED: a classifier error (model down, unparseable
    /// output, timeout) → `Deny`.
    pub async fn review(&self, call: &ToolCall) -> Decision {
        match self.classifier.classify(call, &self.policy).await {
            Ok(risk) => self.thresholds.resolve(risk),
            Err(_) => Decision::Deny,
        }
    }
}

/// The production classifier — a cheap model call that returns one of
/// `LOW`/`HIGH`/`CRITICAL`. Unparseable output is an error → fail-closed `Deny`.
pub struct ModelRiskClassifier {
    pub model: Arc<dyn ModelClient>,
    pub model_name: String,
}

impl ModelRiskClassifier {
    pub fn new(model: Arc<dyn ModelClient>, model_name: impl Into<String>) -> Self {
        Self {
            model,
            model_name: model_name.into(),
        }
    }
}

#[async_trait]
impl RiskClassifier for ModelRiskClassifier {
    async fn classify(&self, call: &ToolCall, policy: &str) -> Result<Risk, AppError> {
        let req = ChatRequest {
            model: self.model_name.clone(),
            messages: vec![
                ChatMessage::system(format!(
                    "You are a security reviewer. Reviewer policy:\n{policy}\n\nClassify the risk \
                     of the tool call for exfiltration / credential-probe / destructive / \
                     persistence. Reply with exactly one word: LOW, HIGH, or CRITICAL."
                )),
                ChatMessage::user(format!(
                    "Tool: {}\nArguments: {}",
                    call.name, call.input
                )),
            ],
            ..Default::default()
        };
        let (msg, _usage) = self.model.call(req).await?;
        let text = msg
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
            .to_uppercase();

        // Order matters: CRITICAL before HIGH before LOW.
        if text.contains("CRITICAL") {
            Ok(Risk::Critical)
        } else if text.contains("HIGH") {
            Ok(Risk::High)
        } else if text.contains("LOW") {
            Ok(Risk::Low)
        } else {
            Err(AppError::internal_error(
                "reviewer: unparseable risk classification",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedClassifier(Option<Risk>);

    #[async_trait]
    impl RiskClassifier for FixedClassifier {
        async fn classify(&self, _call: &ToolCall, _policy: &str) -> Result<Risk, AppError> {
            match self.0 {
                Some(r) => Ok(r),
                None => Err(AppError::internal_error("boom")),
            }
        }
    }

    fn call() -> ToolCall {
        ToolCall {
            id: "1".into(),
            server: Some("external".into()),
            name: "delete_all".into(),
            input: serde_json::json!({}),
        }
    }

    #[test]
    fn risk_maps_to_decision() {
        assert_eq!(map_risk(Risk::Low), Decision::Auto);
        assert_eq!(map_risk(Risk::High), Decision::Prompt);
        assert_eq!(map_risk(Risk::Critical), Decision::Deny);
    }

    #[tokio::test]
    async fn review_low_auto_high_prompt_critical_deny() {
        for (risk, expect) in [
            (Risk::Low, Decision::Auto),
            (Risk::High, Decision::Prompt),
            (Risk::Critical, Decision::Deny),
        ] {
            let rev = Reviewer::new(Arc::new(FixedClassifier(Some(risk))), "policy");
            assert_eq!(rev.review(&call()).await, expect);
        }
    }

    #[test]
    fn thresholds_override_default_ladder() {
        // `{"high":"deny"}` → High resolves to Deny (overriding the default Prompt).
        let t = RiskThresholds::from_json(&serde_json::json!({"high": "deny"}));
        assert!(!t.is_empty());
        assert_eq!(t.resolve(Risk::High), Decision::Deny);
        // Bands the map OMITS keep the default ladder.
        assert_eq!(t.resolve(Risk::Low), Decision::Auto);
        assert_eq!(t.resolve(Risk::Critical), Decision::Deny);
        // Case-insensitive keys + values.
        let t2 = RiskThresholds::from_json(&serde_json::json!({"LOW": "Prompt"}));
        assert_eq!(t2.resolve(Risk::Low), Decision::Prompt);
    }

    #[test]
    fn empty_thresholds_is_default_ladder() {
        // Default (no overrides) reproduces the historical ladder exactly.
        let t = RiskThresholds::default();
        assert!(t.is_empty());
        assert_eq!(t.resolve(Risk::Low), Decision::Auto);
        assert_eq!(t.resolve(Risk::High), Decision::Prompt);
        assert_eq!(t.resolve(Risk::Critical), Decision::Deny);
        // A non-object JSON value also degrades to the default ladder.
        let t2 = RiskThresholds::from_json(&serde_json::json!("nope"));
        assert!(t2.is_empty());
        assert_eq!(t2.resolve(Risk::High), Decision::Prompt);
        // Unknown band / decision names are ignored (default ladder kept).
        let t3 = RiskThresholds::from_json(&serde_json::json!({"medium": "auto", "high": "shrug"}));
        assert!(t3.is_empty());
    }

    #[tokio::test]
    async fn reviewer_consumes_thresholds() {
        // A High classification + `{"high":"deny"}` → the reviewer denies.
        let rev = Reviewer::new_with_thresholds(
            Arc::new(FixedClassifier(Some(Risk::High))),
            "policy",
            RiskThresholds::from_json(&serde_json::json!({"high": "deny"})),
        );
        assert_eq!(rev.review(&call()).await, Decision::Deny);
        // Same classification with DEFAULT thresholds → the default ladder (Prompt).
        let rev_default =
            Reviewer::new(Arc::new(FixedClassifier(Some(Risk::High))), "policy");
        assert_eq!(rev_default.review(&call()).await, Decision::Prompt);
    }

    #[tokio::test]
    async fn review_fails_closed_to_deny() {
        let rev = Reviewer::new(Arc::new(FixedClassifier(None)), "policy");
        assert_eq!(rev.review(&call()).await, Decision::Deny);
    }

    #[tokio::test]
    async fn model_classifier_parses_and_fails_closed() {
        use crate::test_fakes::ScriptedModel;
        // Parses CRITICAL from the model's text.
        let model = Arc::new(ScriptedModel::final_text("This is CRITICAL risk."));
        let clf = ModelRiskClassifier::new(model, "reviewer");
        assert_eq!(clf.classify(&call(), "p").await.unwrap(), Risk::Critical);

        // Unparseable output → error → the Reviewer fails closed to Deny.
        let vague = Arc::new(ScriptedModel::final_text("hmm, not sure"));
        let rev = Reviewer::new(
            Arc::new(ModelRiskClassifier::new(vague, "reviewer")),
            "p",
        );
        assert_eq!(rev.review(&call()).await, Decision::Deny);
    }
}
