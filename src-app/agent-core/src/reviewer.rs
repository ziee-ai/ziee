//! The reviewer flow (ITEM-12, Codex `auto_review`) — a cheap model risk-
//! classifies a tool call that already needs approval, mapping the risk onto a
//! `Decision`. **Fail-closed**: any classifier error → `Deny` (Codex).
//!
//! The loop reaches this only for a `Decision::Review` outcome (the approval
//! matrix, `crate::policy`): `Low → Auto` (proceed), `High → Prompt` (escalate
//! to the durable `HumanGate`), `Critical → Deny`.

use std::sync::Arc;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock};
use async_trait::async_trait;
use ziee_core::AppError;

use crate::core::ModelClient;
use crate::types::{Decision, ToolCall};

/// The risk classes a tool call is sorted into (Codex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    Low,
    High,
    Critical,
}

/// Map a risk class onto the pre-execution decision (Codex mapping).
pub fn map_risk(risk: Risk) -> Decision {
    match risk {
        Risk::Low => Decision::Auto,
        Risk::High => Decision::Prompt,
        Risk::Critical => Decision::Deny,
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
}

impl Reviewer {
    pub fn new(classifier: Arc<dyn RiskClassifier>, policy: impl Into<String>) -> Self {
        Self {
            classifier,
            policy: policy.into(),
        }
    }

    /// Resolve a `Decision::Review` into a concrete `Decision`. FAIL-CLOSED: a
    /// classifier error (model down, unparseable output, timeout) → `Deny`.
    pub async fn review(&self, call: &ToolCall) -> Decision {
        match self.classifier.classify(call, &self.policy).await {
            Ok(risk) => map_risk(risk),
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
