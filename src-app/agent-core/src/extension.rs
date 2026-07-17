//! The `AgentExtension` seam (ITEM-32) — the trait lives in the crate; the ziee
//! server owns the `AGENT_EXTENSIONS` registry (mirroring
//! `ziee_framework::entity_extension`) and injects the ordered list into the
//! loop. Two tiers: core (always-on, e.g. compaction) vs feature (opt-in).

use std::collections::HashMap;
use std::sync::Arc;

use ai_providers::{ChatRequest, ChatMessage, ContentBlock, ContentBlockDelta};
use async_trait::async_trait;
use ziee_core::AppError;

use crate::types::ToolScope;

/// Whether a hook lets the turn proceed or short-circuits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// Stop the turn with a final text (e.g. a policy veto).
    ShortCircuit,
}

/// Per-turn mutable context an extension contributes to (system blocks +
/// tool scope + shared flags). Analogous to chat's `StreamContext`, minus the
/// `PgPool` (which a concrete server-side extension captures in its own field).
#[derive(Debug, Default)]
pub struct TurnContext {
    pub system: Vec<ContentBlock>,
    pub tool_scope: ToolScope,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A pluggable per-turn contributor. Concrete impls live server-side and may
/// capture `Repos`/`PgPool`; only this signature is domain-free.
#[async_trait]
pub trait AgentExtension: Send + Sync {
    fn name(&self) -> &str;
    fn order(&self) -> i32;
    /// Core extensions (compaction) are always registered + non-removable.
    fn is_core(&self) -> bool {
        false
    }

    /// Contribute system blocks / tool scope / flags once per turn.
    async fn contribute(&self, _ctx: &mut TurnContext) -> Result<(), AppError> {
        Ok(())
    }

    /// Mutate the assembled request before each model call; may short-circuit.
    async fn before_model(&self, _req: &mut ChatRequest) -> Result<Flow, AppError> {
        Ok(Flow::Continue)
    }

    /// Post-process each assistant/tool round (e.g. background memory extract).
    async fn after_round(&self, _msg: &ChatMessage) -> Result<Flow, AppError> {
        Ok(Flow::Continue)
    }

    /// Streaming-delta hooks (generalize chat's `process_delta`/`accumulate`/
    /// `get_accumulated_content`) — default no-op.
    async fn on_delta(&self, _delta: &ContentBlockDelta) -> Result<(), AppError> {
        Ok(())
    }
}

/// Run the `contribute` phase across the ordered extension set.
pub async fn run_contribute(
    exts: &[Arc<dyn AgentExtension>],
    ctx: &mut TurnContext,
) -> Result<(), AppError> {
    for ext in exts {
        ext.contribute(ctx).await?;
    }
    Ok(())
}

/// Run `before_model` across the set; returns `ShortCircuit` if any hook vetoes.
pub async fn run_before_model(
    exts: &[Arc<dyn AgentExtension>],
    req: &mut ChatRequest,
) -> Result<Flow, AppError> {
    for ext in exts {
        if ext.before_model(req).await? == Flow::ShortCircuit {
            return Ok(Flow::ShortCircuit);
        }
    }
    Ok(Flow::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AddSystemExt {
        order: i32,
        core: bool,
        tag: &'static str,
    }

    #[async_trait]
    impl AgentExtension for AddSystemExt {
        fn name(&self) -> &str {
            self.tag
        }
        fn order(&self) -> i32 {
            self.order
        }
        fn is_core(&self) -> bool {
            self.core
        }
        async fn contribute(&self, ctx: &mut TurnContext) -> Result<(), AppError> {
            ctx.system.push(ContentBlock::Text {
                text: self.tag.to_string(),
            });
            Ok(())
        }
    }

    struct VetoExt;
    #[async_trait]
    impl AgentExtension for VetoExt {
        fn name(&self) -> &str {
            "veto"
        }
        fn order(&self) -> i32 {
            99
        }
        async fn before_model(&self, _req: &mut ChatRequest) -> Result<Flow, AppError> {
            Ok(Flow::ShortCircuit)
        }
    }

    #[tokio::test]
    async fn contribute_runs_in_registered_order() {
        let exts: Vec<Arc<dyn AgentExtension>> = vec![
            Arc::new(AddSystemExt { order: 10, core: false, tag: "a" }),
            Arc::new(AddSystemExt { order: 20, core: true, tag: "compaction" }),
        ];
        let mut ctx = TurnContext::default();
        run_contribute(&exts, &mut ctx).await.unwrap();
        // Both contributed; a core extension is present in the set.
        assert_eq!(ctx.system.len(), 2);
        assert!(exts.iter().any(|e| e.is_core()));
    }

    #[tokio::test]
    async fn before_model_short_circuits() {
        let exts: Vec<Arc<dyn AgentExtension>> = vec![Arc::new(VetoExt)];
        let mut req = ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("hi")],
            ..Default::default()
        };
        let flow = run_before_model(&exts, &mut req).await.unwrap();
        assert_eq!(flow, Flow::ShortCircuit);
    }
}
