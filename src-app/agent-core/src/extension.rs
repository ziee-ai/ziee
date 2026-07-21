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
    /// Opaque per-turn input bag (DEC-19) — carries the host's request-scoped
    /// extension payload (chat's `SendMessageRequest.extensions`: attach flags,
    /// `file_ids`, `tool_approvals`). The crate never names a field inside it;
    /// a ported extension reads its own key. The workflow host passes `Null`.
    pub inputs: serde_json::Value,
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

/// Order the extension pipeline by [`AgentExtension::order`] ascending, with a
/// **STABLE** sort so insertion order is preserved among extensions that share
/// an `.order()` value (ITEM-56 / DEC-129).
///
/// Historically the loop ran extensions in raw `Vec`-insertion order and never
/// consulted `.order()`, so the tier orders (all `< COMPACTION_ORDER`) and
/// `COMPACTION_ORDER = 1000` were inert. Sorting here makes them load-bearing:
/// cheaper context tiers run before the compaction extension. Returns a
/// cheaply-cloned (`Arc`) ordered copy; the loop calls this ONCE per
/// [`AgentCore::run`](crate::core::AgentCore::run) and reuses it across every
/// iteration (contribute / before_model / after_round).
pub fn sorted_extensions(exts: &[Arc<dyn AgentExtension>]) -> Vec<Arc<dyn AgentExtension>> {
    let mut ordered = exts.to_vec();
    // `slice::sort_by_key` is a STABLE sort — equal `.order()` keep insertion order.
    ordered.sort_by_key(|e| e.order());
    ordered
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

    #[test]
    fn sorted_extensions_orders_ascending_and_stable() {
        // Inserted OUT of order, with two extensions sharing order 20.
        let exts: Vec<Arc<dyn AgentExtension>> = vec![
            Arc::new(AddSystemExt { order: 30, core: false, tag: "c30" }),
            Arc::new(AddSystemExt { order: 10, core: false, tag: "a10" }),
            Arc::new(AddSystemExt { order: 20, core: false, tag: "b20_first" }),
            Arc::new(AddSystemExt { order: 20, core: false, tag: "b20_second" }),
        ];
        let ordered = sorted_extensions(&exts);
        let tags: Vec<&str> = ordered.iter().map(|e| e.name()).collect();
        // Low `.order()` before high regardless of insertion; equal orders keep
        // insertion order (stable): b20_first before b20_second.
        assert_eq!(tags, vec!["a10", "b20_first", "b20_second", "c30"]);
    }

    #[tokio::test]
    async fn contribute_runs_in_order_not_insertion_order() {
        // A high-order extension inserted FIRST must still contribute AFTER a
        // low-order one inserted later, once sorted.
        let exts: Vec<Arc<dyn AgentExtension>> = vec![
            Arc::new(AddSystemExt { order: 1000, core: true, tag: "compaction" }),
            Arc::new(AddSystemExt { order: 5, core: false, tag: "early" }),
        ];
        let ordered = sorted_extensions(&exts);
        let mut ctx = TurnContext::default();
        run_contribute(&ordered, &mut ctx).await.unwrap();
        let seen: Vec<&str> = ctx
            .system
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(seen, vec!["early", "compaction"]);
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
