//! Workflow runner (plan §4.0 + §4.5).
//!
//! Entry: `run_workflow(pool, run_id)` — tokio task spawned by the
//! `POST /run` handler. Walks the steps in topo order, dispatches each
//! via `StepDispatcher`, persists per-step metadata via repository
//! helpers, emits per-step events via the registry's mpsc fan-out.
//!
//! Wall-clock cap: 30 min via `tokio::time::timeout` wrapping the
//! whole runner future.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::workflow::artifact_io;
use crate::modules::workflow::dispatch::{
    ElicitDispatcher, LlmDispatcher, LlmMapDispatcher, SandboxDispatcher, StepDispatcher,
};
use crate::modules::workflow::events::{
    PerRunEmitter, ProgressEmitter, SSEElicitationResolvedData, SSERunCancelledData,
    SSERunCompletedData, SSERunFailedData, SSERunStartedData, SSEStepCompletedData,
    SSEStepFailedData, SSEStepStartedData, SSEWorkflowRunEvent,
};
use crate::modules::workflow::file_io;
use crate::modules::workflow::log_io::{self, StepTrace};
use crate::modules::workflow::models::WorkflowRunStatus;
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{
    ItemProgress, OutputMeta, ParsedAs, RunContext, StepKindTag, StepResult,
};
use crate::modules::workflow::validate::{
    OutputDef, StepConfig, StepDef, WorkflowDef, parse_workflow_yaml, topo_sort_steps,
};

/// Global per-run wall-clock cap (30 min). The workflow runner stays
/// inside this; any LLM call or sandbox exec that takes longer fails
/// the run with a `wall_clock_exceeded` error message.
pub const RUN_WALL_CLOCK: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Per-run cumulative token cap (plan §4.5).
pub const PER_RUN_TOKEN_CAP: u64 = 5_000_000;

/// Pre-flight: parse + validate inputs + build the `RunContext`.
/// Called by the route handler BEFORE spawning the runner task.
pub async fn preflight(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    workflow_id: Uuid,
    inputs: Value,
    workflow: &WorkflowDef,
    extracted_path: PathBuf,
    workspace_root: PathBuf,
    model_id: Uuid,
    model_name: String,
    sandbox_flavor: Option<String>,
    is_dev: bool,
    mocks: HashMap<String, Value>,
) -> Result<RunContext, AppError> {
    // Validate `inputs` against workflow.inputs[].
    let mut bound: HashMap<String, Value> = HashMap::new();
    let provided_obj = match inputs {
        Value::Object(m) => m,
        Value::Null => Default::default(),
        _ => {
            return Err(AppError::bad_request(
                "WORKFLOW_INPUTS_NOT_OBJECT",
                "request inputs must be a JSON object",
            ));
        }
    };
    for input in &workflow.inputs {
        if let Some(v) = provided_obj.get(&input.name) {
            bound.insert(input.name.clone(), v.clone());
        } else if let Some(d) = &input.default {
            bound.insert(input.name.clone(), d.clone());
        } else if input.required {
            return Err(AppError::bad_request(
                "WORKFLOW_INPUT_MISSING",
                format!("required input '{}' not provided", input.name),
            ));
        }
    }

    // Stage workspace dir: `<workspace_root>/<conv-or-run-id>/workflow/<run_id>/`.
    let conv_dir_id = conversation_id.unwrap_or(run_id);
    let sandbox_workspace = workspace_root
        .join(conv_dir_id.to_string())
        .join("workflow")
        .join(run_id.to_string());
    tokio::fs::create_dir_all(&sandbox_workspace)
        .await
        .map_err(|e| {
            AppError::internal_error(format!(
                "workflow runner: mkdir staged dir {}: {e}",
                sandbox_workspace.display()
            ))
        })?;
    let outputs_dir = sandbox_workspace.join("outputs");
    let artifacts_dir = sandbox_workspace.join("artifacts");
    let inputs_dir = sandbox_workspace.join("inputs");
    for d in [&outputs_dir, &artifacts_dir, &inputs_dir] {
        tokio::fs::create_dir_all(d).await.map_err(|e| {
            AppError::internal_error(format!("workflow runner: mkdir {}: {e}", d.display()))
        })?;
    }

    // If `kind: sandbox` exists: copy bundle's scripts/ + prompts/ +
    // references/ into the staged dir (RO mount source).
    let has_sandbox = workflow
        .steps
        .iter()
        .any(|s| matches!(s.config, StepConfig::Sandbox { .. }));
    if has_sandbox {
        for sub in &["scripts", "prompts", "references"] {
            let src = extracted_path.join(sub);
            if src.exists() {
                let dst = sandbox_workspace.join(sub);
                copy_dir_recursive(&src, &dst).await?;
            }
        }
    }

    // Note: this just builds the in-memory struct. The DB row is
    // already inserted by the handler before preflight runs.
    let _ = pool; // pool used by handler, not here.
    Ok(RunContext {
        run_id,
        user_id,
        conversation_id,
        workflow_id,
        inputs: bound,
        step_outputs: HashMap::new(),
        step_item_progress: HashMap::new(),
        extracted_path,
        sandbox_workspace,
        outputs_dir,
        artifacts_dir,
        inputs_dir,
        model_id,
        model_name,
        sandbox_flavor,
        total_tokens: 0,
        is_dev,
        // Mocks only honored for dev workflows. The handler already gates
        // this (403 when mocks present on a published workflow), but
        // belt-and-suspenders: drop them here too if somehow non-dev.
        mocks: if is_dev { mocks } else { HashMap::new() },
    })
}

async fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), AppError> {
    use std::fs;
    tokio::task::block_in_place(|| -> Result<(), AppError> {
        fs::create_dir_all(dst).map_err(|e| {
            AppError::internal_error(format!("mkdir {}: {e}", dst.display()))
        })?;
        for entry in fs::read_dir(src).map_err(|e| {
            AppError::internal_error(format!("read_dir {}: {e}", src.display()))
        })? {
            let entry = entry.map_err(|e| AppError::internal_error(format!("entry: {e}")))?;
            let md = entry.metadata().map_err(|e| AppError::internal_error(format!("stat: {e}")))?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if md.is_dir() {
                // Recurse using a stack so we don't need tokio::fs in nested closures.
                std::fs::create_dir_all(&to).ok();
                let mut stack = vec![(from, to)];
                while let Some((s, d)) = stack.pop() {
                    std::fs::create_dir_all(&d).ok();
                    for e in std::fs::read_dir(&s).map_err(|e| AppError::internal_error(format!("read_dir: {e}")))? {
                        let e = e.map_err(|e| AppError::internal_error(format!("entry: {e}")))?;
                        let m = e.metadata().map_err(|e| AppError::internal_error(format!("stat: {e}")))?;
                        let f = e.path();
                        let t = d.join(e.file_name());
                        if m.is_dir() {
                            stack.push((f, t));
                        } else if m.is_file() {
                            std::fs::copy(&f, &t).map_err(|e| AppError::internal_error(format!("copy {} -> {}: {e}", f.display(), t.display())))?;
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let _ = std::fs::set_permissions(&t, std::fs::Permissions::from_mode(m.permissions().mode()));
                            }
                        }
                    }
                }
            } else if md.is_file() {
                fs::copy(&from, &to).map_err(|e| {
                    AppError::internal_error(format!(
                        "copy {} -> {}: {e}",
                        from.display(),
                        to.display()
                    ))
                })?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        fs::set_permissions(&to, fs::Permissions::from_mode(md.permissions().mode()));
                }
            }
        }
        Ok(())
    })
}

/// Dev-only mock short-circuit. Writes the canned `mock` value as the
/// step's output (same file path + metadata the real dispatchers would
/// produce) so downstream `{{ step.output }}` template refs resolve, then
/// returns `Completed` with zero tokens. A JSON object/array/number/bool
/// is stored as `parsed_as: json`; a string is stored as `text`.
async fn run_mock_step(ctx: &mut RunContext, step_id: &str, mock: Value) -> StepResult {
    let started = std::time::Instant::now();
    let parsed_as = match &mock {
        Value::String(_) => ParsedAs::Text,
        _ => ParsedAs::Json,
    };
    let meta = match crate::modules::workflow::file_io::write_step_output(
        ctx,
        step_id,
        &mock,
        parsed_as,
        StepKindTag::Llm,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            return StepResult::Failed {
                error: format!("mock write failed: {e}"),
                tokens_used: 0,
            };
        }
    };
    ctx.step_outputs.insert(step_id.to_string(), meta);
    StepResult::Completed {
        output: mock,
        parsed_as,
        tokens_used: 0,
        ms_elapsed: started.elapsed().as_millis() as u64,
    }
}

/// Tokio task entry: dispatches each step, persists metadata, emits
/// events. Returns Ok on terminal status; the only Err it returns is
/// catastrophic (e.g. failed to mark status — the runner already wrote
/// `failed` in that case).
pub async fn run_workflow(
    pool: PgPool,
    mut ctx: RunContext,
    workflow_def: WorkflowDef,
    provider: Arc<ai_providers::Provider>,
) {
    let run_id = ctx.run_id;
    let user_id = ctx.user_id;
    let started = Instant::now();
    let handle = match registry::get(run_id) {
        Some(h) => h,
        None => {
            // Handler should have registered. Defensive — register on the fly.
            registry::register(run_id)
        }
    };
    let emit: Arc<dyn ProgressEmitter> = Arc::new(PerRunEmitter { run_id });

    // Wrap the entire run in the wall-clock timeout.
    let outcome = tokio::time::timeout(
        RUN_WALL_CLOCK,
        run_inner(&pool, &mut ctx, &workflow_def, provider, handle.clone(), emit.clone()),
    )
    .await;

    let total_tokens = ctx.total_tokens;

    let final_outcome = match outcome {
        Ok(r) => r,
        Err(_) => RunInnerOutcome::Failed {
            error: "workflow runner wall-clock timeout (30 min)".into(),
            failed_at_step: None,
        },
    };

    match final_outcome {
        RunInnerOutcome::Completed { outputs_preview } => {
            let _ = repository::mark_status(
                &pool,
                run_id,
                WorkflowRunStatus::Completed,
                None,
            )
            .await;
            emit.emit(SSEWorkflowRunEvent::RunCompleted(SSERunCompletedData {
                run_id,
                outputs_preview,
                total_tokens,
                ms_elapsed: started.elapsed().as_millis() as u64,
            }));
            crate::modules::workflow::events::emit_workflow_run(
                crate::modules::sync::SyncAction::Update,
                run_id,
                user_id,
                None,
            );
        }
        RunInnerOutcome::Cancelled {
            cancelled_at_step,
        } => {
            // The cancel handler may have already flipped status; this is idempotent.
            let _ = repository::mark_status(
                &pool,
                run_id,
                WorkflowRunStatus::Cancelled,
                Some("cancelled by user"),
            )
            .await;
            emit.emit(SSEWorkflowRunEvent::RunCancelled(SSERunCancelledData {
                run_id,
                cancelled_at_step,
                total_tokens,
                tokens_at_cancel: total_tokens,
            }));
            crate::modules::workflow::events::emit_workflow_run(
                crate::modules::sync::SyncAction::Update,
                run_id,
                user_id,
                None,
            );
        }
        RunInnerOutcome::Failed {
            error,
            failed_at_step,
        } => {
            let _ = repository::mark_status(
                &pool,
                run_id,
                WorkflowRunStatus::Failed,
                Some(&error),
            )
            .await;
            emit.emit(SSEWorkflowRunEvent::RunFailed(SSERunFailedData {
                run_id,
                error,
                total_tokens,
                failed_at_step,
            }));
            crate::modules::workflow::events::emit_workflow_run(
                crate::modules::sync::SyncAction::Update,
                run_id,
                user_id,
                None,
            );
        }
    }

    // Cleanup the staged dir.
    let _ = tokio::fs::remove_dir_all(&ctx.sandbox_workspace).await;
    registry::unregister(run_id);
}

#[derive(Debug)]
enum RunInnerOutcome {
    Completed { outputs_preview: Value },
    Cancelled { cancelled_at_step: Option<String> },
    Failed { error: String, failed_at_step: Option<String> },
}

async fn run_inner(
    pool: &PgPool,
    ctx: &mut RunContext,
    workflow: &WorkflowDef,
    provider: Arc<ai_providers::Provider>,
    handle: Arc<registry::RunHandle>,
    emit: Arc<dyn ProgressEmitter>,
) -> RunInnerOutcome {
    let _ = repository::mark_running(pool, ctx.run_id).await;
    emit.emit(SSEWorkflowRunEvent::RunStarted(SSERunStartedData {
        run_id: ctx.run_id,
        workflow_id: ctx.workflow_id,
        model_id: Some(ctx.model_id),
        sandbox_flavor: ctx.sandbox_flavor.clone(),
        total_steps: workflow.steps.len() as u32,
        conversation_id: ctx.conversation_id,
    }));
    crate::modules::workflow::events::emit_workflow_run(
        crate::modules::sync::SyncAction::Create,
        ctx.run_id,
        ctx.user_id,
        None,
    );

    let order = match topo_sort_steps(workflow) {
        Ok(o) => o,
        Err(e) => {
            return RunInnerOutcome::Failed {
                error: format!("topo-sort: {e}"),
                failed_at_step: None,
            };
        }
    };

    let total_steps = order.len() as u32;
    for (i, step_idx) in order.iter().enumerate() {
        let step = &workflow.steps[*step_idx];
        let step_started = Utc::now();

        if handle.is_cancelled() {
            return RunInnerOutcome::Cancelled {
                cancelled_at_step: Some(step.id.clone()),
            };
        }

        let message_rendered = step
            .message
            .as_deref()
            .and_then(|m| crate::modules::workflow::template::render(m, ctx).ok());

        emit.emit(SSEWorkflowRunEvent::StepStarted(SSEStepStartedData {
            run_id: ctx.run_id,
            step_id: step.id.clone(),
            step_kind: step.config.kind_str().to_string(),
            step_index: i as u32,
            total_steps,
            message: message_rendered,
        }));

        // Mock short-circuit (dev only). Honor a per-run `mocks[step.id]`
        // from the /run body OR a `StepDef.mock` baked into the workflow.
        // Skips real dispatch entirely — no LLM tokens, no sandbox spawn.
        // Gated on `is_dev` (handler rejects mocks for published workflows,
        // and RunContext drops them when !is_dev). See plan §1 + B4 audit.
        let mock_value: Option<Value> = if ctx.is_dev {
            ctx.mocks
                .get(&step.id)
                .cloned()
                .or_else(|| step.mock.clone())
        } else {
            None
        };

        let result = if let Some(mv) = mock_value {
            run_mock_step(ctx, &step.id, mv).await
        } else {
            let dispatcher: Box<dyn StepDispatcher> = match &step.config {
                StepConfig::Llm { .. } => Box::new(LlmDispatcher::new(provider.clone())),
                StepConfig::LlmMap { .. } => Box::new(LlmMapDispatcher::new(provider.clone())),
                StepConfig::Sandbox { .. } => Box::new(SandboxDispatcher::new()),
                StepConfig::Elicit { .. } => Box::new(ElicitDispatcher::new()),
            };
            tokio::select! {
                r = dispatcher.dispatch(step, ctx, handle.clone(), emit.clone()) => r,
                _ = handle.await_cancel() => StepResult::Cancelled,
            }
        };

        match result {
            StepResult::Completed { output, parsed_as, tokens_used, ms_elapsed } => {
                // Persist meta (already wrote the file).
                if let Some(meta) = ctx.step_outputs.get(&step.id).cloned() {
                    let meta_json = serde_json::to_value(&meta).unwrap_or(Value::Null);
                    let _ = repository::persist_step_meta(
                        pool,
                        ctx.run_id,
                        &step.id,
                        &meta_json,
                        tokens_used,
                        Some(&step.id),
                    )
                    .await;
                }
                // Collect step artifacts (sandbox steps only).
                if matches!(step.config, StepConfig::Sandbox { .. }) {
                    let artifacts = artifact_io::collect_step_artifacts(ctx, step)
                        .unwrap_or_default();
                    if !artifacts.is_empty() {
                        let json = serde_json::to_value(&artifacts).unwrap_or(Value::Null);
                        let _ = repository::persist_step_artifacts(
                            pool,
                            ctx.run_id,
                            &step.id,
                            &json,
                        )
                        .await;
                    }
                }
                // Persist item progress if any.
                if let Some(p) = ctx.step_item_progress.get(&step.id).cloned() {
                    let pj = serde_json::to_value(&p).unwrap_or(Value::Null);
                    let _ = repository::persist_step_item_progress(
                        pool,
                        ctx.run_id,
                        &step.id,
                        &pj,
                    )
                    .await;
                }
                // Write per-step trace log.
                let trace = StepTrace {
                    started_at: Some(step_started),
                    completed_at: Some(Utc::now()),
                    ms_elapsed,
                    tokens_used,
                    attempts: 1,
                    on_error: None,
                };
                let _ = log_io::write_trace(ctx, &step.id, &trace).await;

                let preview = ctx
                    .step_outputs
                    .get(&step.id)
                    .map(|m| m.preview.clone())
                    .unwrap_or_default();
                emit.emit(SSEWorkflowRunEvent::StepCompleted(SSEStepCompletedData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    output_preview: preview,
                    tokens_used,
                    ms_elapsed,
                }));
                // Token cap check.
                if ctx.total_tokens > PER_RUN_TOKEN_CAP {
                    return RunInnerOutcome::Failed {
                        error: format!(
                            "per-run token cap {} exceeded ({} used)",
                            PER_RUN_TOKEN_CAP, ctx.total_tokens
                        ),
                        failed_at_step: Some(step.id.clone()),
                    };
                }
                let _ = output;
                let _ = parsed_as;
            }
            StepResult::Failed { error, tokens_used } => {
                emit.emit(SSEWorkflowRunEvent::StepFailed(SSEStepFailedData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    error: error.clone(),
                    tokens_used,
                }));
                return RunInnerOutcome::Failed {
                    error: format!("step '{}' failed: {error}", step.id),
                    failed_at_step: Some(step.id.clone()),
                };
            }
            StepResult::Cancelled => {
                return RunInnerOutcome::Cancelled {
                    cancelled_at_step: Some(step.id.clone()),
                };
            }
        }

        // Check DB-side cancel flip between steps (cheap safety net).
        if let Ok(Some(r)) = repository::find_run(pool, ctx.run_id).await
            && r.status == "cancelled"
        {
            return RunInnerOutcome::Cancelled {
                cancelled_at_step: Some(step.id.clone()),
            };
        }
    }

    // Resolve outputs[].
    let outputs_preview = match resolve_outputs(ctx, &workflow.outputs).await {
        Ok(v) => v,
        Err(e) => {
            return RunInnerOutcome::Failed {
                error: format!("output resolution: {e}"),
                failed_at_step: None,
            };
        }
    };
    // Persist final_output_json.
    let _ = repository::set_final_output(pool, ctx.run_id, outputs_preview.clone()).await;
    RunInnerOutcome::Completed { outputs_preview }
}

async fn resolve_outputs(
    ctx: &mut RunContext,
    outputs: &[OutputDef],
) -> Result<Value, AppError> {
    let mut map = serde_json::Map::new();
    for o in outputs {
        let rendered = crate::modules::workflow::template::render(&o.from, ctx)
            .map_err(|e| AppError::internal_error(format!("output '{}': {e}", o.name)))?;
        // The render returns a string; preview cap.
        let truncated = if rendered.len() > 500 {
            format!("{}…", &rendered[..500])
        } else {
            rendered.clone()
        };
        map.insert(
            o.name.clone(),
            serde_json::json!({
                "value_preview": truncated,
                "size_bytes": rendered.len(),
                "expose": format!("{:?}", o.expose).to_lowercase(),
            }),
        );
    }
    Ok(Value::Object(map))
}

/// Shared run-spawn path used by BOTH the REST `POST /run` handler and
/// the `workflow_mcp` tool-call path (B5). Loads + validates the
/// workflow.yaml, snapshots the conversation's model, inserts the
/// `workflow_runs` row, registers the run handle, runs `preflight`,
/// resolves the provider, and spawns the runner task. Returns the
/// created `run_id` immediately (fire-and-forget); callers that need to
/// block until completion (workflow_mcp) poll `repository::find_run`.
///
/// `mocks` are dev-only and already gated by the caller (REST handler
/// rejects mocks on a non-dev workflow with 403; preflight drops them
/// when `!is_dev`).
pub async fn spawn_run(
    pool: &PgPool,
    workflow: &crate::modules::workflow::models::Workflow,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    inputs: Value,
    mocks: HashMap<String, Value>,
) -> Result<Uuid, AppError> {
    if !workflow.enabled {
        return Err(AppError::bad_request(
            "WORKFLOW_DISABLED",
            "workflow is disabled",
        ));
    }

    // Parse + validate the on-disk workflow.yaml.
    let wf_yaml_path = PathBuf::from(&workflow.extracted_path).join(&workflow.entry_point);
    let content = tokio::fs::read_to_string(&wf_yaml_path).await.map_err(|e| {
        AppError::internal_error(format!(
            "workflow: read workflow.yaml at {}: {e}",
            wf_yaml_path.display()
        ))
    })?;
    let workflow_def = crate::modules::workflow::validate::parse_workflow_yaml(&content)?;
    crate::modules::workflow::validate::validate_for_install(
        &workflow_def,
        std::path::Path::new(&workflow.extracted_path),
        workflow.is_dev,
    )?;

    // Snapshot the model from the conversation (Phase 1: a conversation
    // is required so we have a model to run llm steps with).
    let conv_id = conversation_id.ok_or_else(|| {
        AppError::bad_request(
            "WORKFLOW_NO_MODEL_SOURCE",
            "Phase 1: workflow runs must carry a conversation_id (used to snapshot the model)",
        )
    })?;
    let conv = crate::core::Repos
        .chat
        .core
        .get_conversation(conv_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;
    let model_id = conv.model_id.ok_or_else(|| {
        AppError::bad_request(
            "WORKFLOW_CONVERSATION_NO_MODEL",
            "conversation has no model set; cannot snapshot for workflow run",
        )
    })?;
    let model = crate::core::Repos
        .llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;
    let model_name = model.name.clone();

    let sandbox_flavor = workflow_def.sandbox.as_ref().map(|s| s.flavor.clone());

    let row = repository::insert_run(
        pool,
        crate::modules::workflow::models::CreateWorkflowRun {
            workflow_id: workflow.id,
            conversation_id,
            user_id,
            model_id: Some(model_id),
            sandbox_flavor: sandbox_flavor.clone(),
            run_kind: "normal".into(),
            inputs_json: inputs.clone(),
        },
    )
    .await?;

    let _handle = registry::register(row.id);

    let workspace_root = workflow_workspace_root();
    let ctx = preflight(
        pool,
        row.id,
        user_id,
        conversation_id,
        workflow.id,
        inputs,
        &workflow_def,
        PathBuf::from(&workflow.extracted_path),
        workspace_root,
        model_id,
        model_name,
        sandbox_flavor,
        workflow.is_dev,
        mocks,
    )
    .await?;

    let (provider, _name, _mid, _pid, _params) =
        crate::modules::chat::core::ai_provider::create_provider_from_model_id(model_id, user_id)
            .await?;

    let pool_for_task = pool.clone();
    tokio::spawn(async move {
        run_workflow(pool_for_task, ctx, workflow_def, provider).await;
    });

    Ok(row.id)
}

/// Convenience: lookup the workspace root from the configured
/// `code_sandbox` state (workflow stage dirs live under it). Falls
/// back to a sensible default if the sandbox module isn't initialized
/// (workflow runs WITHOUT sandbox steps still need a workspace).
pub fn workflow_workspace_root() -> PathBuf {
    if let Some(state) = crate::modules::code_sandbox::config::get_state() {
        state.workspace_root.clone()
    } else {
        // Fallback: under /tmp. The runner will never write outside
        // this dir; the bundle dir itself lives elsewhere
        // (`<data_dir>/workflows/...`).
        std::env::temp_dir().join("ziee-workflows")
    }
}
