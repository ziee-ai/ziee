//! Subagent parallel fan-out (ITEM-7, P9) — `AgentCore::fan_out`.
//!
//! A subagent is a fresh agent loop with its OWN context window that returns
//! ONLY a summary (Anthropic / Codex). Fan-out spawns ≤ `max_threads` child
//! cores concurrently (a `tokio::sync::Semaphore`), each with a fresh `run_id`,
//! each resolving its per-child `model_id` via the injected `ModelResolver`
//! (RBAC-bound) when set, and each running with `allow_delegate = false` —
//! structurally enforcing `max_depth = 1` (Codex: no grandchildren). The return
//! contract is `Vec<SubagentSummary>`: never a child transcript.

use std::sync::Arc;

use ai_providers::{ChatMessage, ContentBlock, Role};
use tokio::sync::Semaphore;
use uuid::Uuid;
use ziee_core::AppError;

use crate::core::{AgentCore, CancelToken};
use crate::types::{
    AgentEvent, AgentTurnRequest, SubagentSpec, SubagentSummary, ToolScope, TurnSeed,
};

impl AgentCore {
    /// Spawn N isolated child cores concurrently, bounded by
    /// `SubagentLimits.max_threads`. Each child resolves its `model_id` (when
    /// set) via the `ModelResolver`; a rejected id fails the whole fan-out.
    /// Returns only summaries (P9). `user_id` binds RBAC for model resolution.
    pub async fn fan_out(
        &self,
        user_id: Uuid,
        children: Vec<SubagentSpec>,
        cancel: CancelToken,
    ) -> Result<Vec<SubagentSummary>, AppError> {
        let permits = self.limits.max_threads.max(1) as usize;
        let sem = Arc::new(Semaphore::new(permits));
        let mut handles = Vec::new();

        for spec in children {
            // Resolve the per-child model (RBAC-bound); a rejected id errors out.
            let model_client = match spec.model_id {
                Some(model_id) => {
                    let provider = self.models.resolve(model_id, user_id).await?;
                    self.model_factory.for_provider(provider)
                }
                None => self.model.clone(),
            };

            let mut child = self.clone();
            child.model = model_client;

            let child_req = AgentTurnRequest {
                run_id: Uuid::new_v4(),
                user_id,
                seed: TurnSeed::NewMessage(ChatMessage::user("Proceed with your task.")),
                system: vec![ContentBlock::Text { text: spec.system }],
                tool_scope: ToolScope {
                    servers: spec.tool_scope.servers,
                    // Children never get `delegate` → enforces max_depth = 1.
                    allow_delegate: false,
                },
                start_iteration: 1,
            };

            let sem = sem.clone();
            let cancel = cancel.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem
                    .acquire_owned()
                    .await
                    .map_err(|e| AppError::internal_error(format!("semaphore closed: {e}")))?;
                let events = child.run(child_req, cancel).await?;
                Ok::<SubagentSummary, AppError>(summary_from_events(&events))
            }));
        }

        let mut out = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(summary)) => out.push(summary),
                Ok(Err(e)) => return Err(e),
                Err(e) => {
                    return Err(AppError::internal_error(format!(
                        "subagent task panicked: {e}"
                    )))
                }
            }
        }
        Ok(out)
    }
}

/// The child's summary is its FINAL assistant text — never the transcript (P9).
fn summary_from_events(events: &[AgentEvent]) -> SubagentSummary {
    let mut summary = String::new();
    for ev in events {
        if let AgentEvent::Message(msg) = ev {
            if msg.role == Role::Assistant {
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.is_empty() {
                    summary = text;
                }
            }
        }
    }
    SubagentSummary { summary }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::Budget;
    use crate::core::{ModelClient, ProviderModelClientFactory};
    use crate::policy::TrustedAutoApprovePolicy;
    use crate::test_fakes::{
        FakeFactory, FakeGate, FakeResolver, FakeSink, FakeTools, FakeTranscript, GateBehavior,
        ScriptedModel,
    };
    use crate::types::{ApprovalMode, SandboxMode, SubagentLimits};

    /// Build a fan-out core with a given resolver, factory, model client, and
    /// `max_threads`.
    fn fanout_core(
        model: Arc<dyn ModelClient>,
        resolver: Arc<FakeResolver>,
        factory: Arc<dyn crate::core::ModelClientFactory>,
        max_threads: u8,
    ) -> AgentCore {
        AgentCore {
            transcript: Arc::new(FakeTranscript::default()),
            sink: Arc::new(FakeSink::default()),
            tools: Arc::new(FakeTools::new(true)),
            gate: Arc::new(FakeGate {
                behavior: GateBehavior::Approve,
            }),
            policy: Arc::new(TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest)),
            models: resolver,
            model,
            model_factory: factory,
            extensions: vec![],
            reviewer: None,
            budget: Budget::new(4, 1_000_000, 1_000_000),
            limits: SubagentLimits {
                max_depth: 1,
                max_threads,
            },
            sandbox: SandboxMode::WorkspaceWrite { network: false },
            model_name: "test".into(),
        }
    }

    fn spec(model_id: Option<Uuid>, system: &str) -> SubagentSpec {
        SubagentSpec {
            model_id,
            system: system.into(),
            tool_scope: ToolScope::default(),
            reasoning_effort: None,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrency_is_bounded_by_max_threads() {
        // A shared model with a per-call delay; all children (model_id=None)
        // share it, so its peak-concurrency atomic reflects true parallelism.
        let model = Arc::new(ScriptedModel::concurrent("child done", 30));
        let core = fanout_core(
            model.clone(),
            Arc::new(FakeResolver::default()),
            Arc::new(ProviderModelClientFactory),
            2,
        );
        let children: Vec<_> = (0..5).map(|i| spec(None, &format!("child {i}"))).collect();

        let summaries = core
            .fan_out(Uuid::new_v4(), children, CancelToken::new())
            .await
            .unwrap();

        assert_eq!(summaries.len(), 5);
        // Never more than max_threads model calls in flight at once.
        assert!(model.peak.load(std::sync::atomic::Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn returns_summaries_not_transcripts() {
        let model = Arc::new(ScriptedModel::final_text("the child's final answer"));
        let core = fanout_core(
            model,
            Arc::new(FakeResolver::default()),
            Arc::new(ProviderModelClientFactory),
            6,
        );
        let summaries = core
            .fan_out(Uuid::new_v4(), vec![spec(None, "task")], CancelToken::new())
            .await
            .unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].summary, "the child's final answer");
    }

    #[tokio::test]
    async fn distinct_model_ids_resolve_distinct_providers() {
        let resolver = Arc::new(FakeResolver::default());
        // The factory ignores the resolved provider (network-free) and hands
        // back a fake model client so the child loop can run.
        let factory = Arc::new(FakeFactory {
            inner: Arc::new(ScriptedModel::final_text("done")),
        });
        let core = fanout_core(
            Arc::new(ScriptedModel::final_text("parent-unused")),
            resolver.clone(),
            factory,
            6,
        );

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        core.fan_out(
            Uuid::new_v4(),
            vec![spec(Some(id_a), "a"), spec(Some(id_b), "b")],
            CancelToken::new(),
        )
        .await
        .unwrap();

        // The resolver was asked for each child's distinct model_id.
        let asked = resolver.asked.lock().unwrap().clone();
        assert_eq!(asked.len(), 2);
        assert!(asked.contains(&id_a));
        assert!(asked.contains(&id_b));
        assert_ne!(asked[0], asked[1]);
    }

    #[tokio::test]
    async fn rejected_model_id_errors() {
        let bad = Uuid::new_v4();
        let resolver = Arc::new(FakeResolver {
            asked: Default::default(),
            reject: Some(bad),
        });
        let core = fanout_core(
            Arc::new(ScriptedModel::final_text("x")),
            resolver,
            Arc::new(FakeFactory {
                inner: Arc::new(ScriptedModel::final_text("x")),
            }),
            6,
        );
        let err = core
            .fan_out(Uuid::new_v4(), vec![spec(Some(bad), "a")], CancelToken::new())
            .await;
        assert!(err.is_err());
    }
}
