//! The approval matrix (ITEM-11) — `SandboxMode × ApprovalMode → Decision`
//! (Codex analog). Read-only/trusted built-ins auto-approve; mutating/external
//! calls route to Prompt or Review (the reviewer, DEC-3, sits behind `Review`).

use async_trait::async_trait;

use crate::ports::ApprovalPolicy;
use crate::types::{ApprovalMode, Decision, SandboxMode, ToolCall};

/// The default policy: auto-approve trusted read-only tools; compose the mode
/// for the rest. The reviewer is layered on top of a `Review` outcome by the
/// loop (ITEM-12).
#[derive(Debug, Clone)]
pub struct TrustedAutoApprovePolicy {
    pub mode: ApprovalMode,
}

impl TrustedAutoApprovePolicy {
    pub fn new(mode: ApprovalMode) -> Self {
        Self { mode }
    }

    /// Pure decision fn (testable without async).
    pub fn decide_sync(&self, trusted: bool) -> Decision {
        if trusted {
            // Read-only / trusted built-ins always auto-approve (skip reviewer).
            return Decision::Auto;
        }
        match self.mode {
            // No prompts; a disallowed mutating call fails back to the model.
            ApprovalMode::Never => Decision::Deny,
            // Everything non-trusted asks a human directly.
            ApprovalMode::UnlessTrusted => Decision::Prompt,
            // Default: send to the reviewer, which auto-resolves low risk and
            // escalates high risk to the human gate (DEC-1).
            ApprovalMode::OnRequest => Decision::Review,
            // Per-category; unflagged categories go through the reviewer.
            ApprovalMode::Granular => Decision::Review,
        }
    }
}

#[async_trait]
impl ApprovalPolicy for TrustedAutoApprovePolicy {
    async fn decide(&self, _call: &ToolCall, trusted: bool, _sandbox: &SandboxMode) -> Decision {
        self.decide_sync(trusted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_always_auto() {
        for mode in [
            ApprovalMode::Never,
            ApprovalMode::UnlessTrusted,
            ApprovalMode::OnRequest,
            ApprovalMode::Granular,
        ] {
            assert_eq!(
                TrustedAutoApprovePolicy::new(mode).decide_sync(true),
                Decision::Auto
            );
        }
    }

    #[test]
    fn mutating_matrix() {
        assert_eq!(
            TrustedAutoApprovePolicy::new(ApprovalMode::Never).decide_sync(false),
            Decision::Deny
        );
        assert_eq!(
            TrustedAutoApprovePolicy::new(ApprovalMode::UnlessTrusted).decide_sync(false),
            Decision::Prompt
        );
        assert_eq!(
            TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest).decide_sync(false),
            Decision::Review
        );
    }

    #[tokio::test]
    async fn async_decide_matches_sync() {
        let p = TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest);
        let call = ToolCall {
            id: "1".into(),
            server: Some("web_search".into()),
            name: "search".into(),
            input: serde_json::json!({}),
        };
        let d = p
            .decide(&call, false, &SandboxMode::WorkspaceWrite { network: false })
            .await;
        assert_eq!(d, Decision::Review);
    }
}
