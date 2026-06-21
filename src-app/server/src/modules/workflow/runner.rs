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
    ToolDispatcher,
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

/// Liveness-heartbeat cadence. While a run is in-flight the runner bumps
/// `workflow_runs.updated_at` this often so the workflow_mcp tool path's
/// no-progress guard (5 min) doesn't false-kill a long-but-live single step
/// (a 30-min elicit wait or a 10-min sandbox step produces no step
/// transitions to advance `updated_at` on its own).
pub const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Per-run cumulative token cap (plan §4.5).
pub const PER_RUN_TOKEN_CAP: u64 = 5_000_000;

/// Per-step token cap (plan §4.5 + §10). Aggregate across all `llm_map`
/// items in a single step (a single `llm` step's one call is already
/// ≤ `PER_CALL_TOKEN_CAP` = 50k so it can't reach this, but the runner
/// enforces it uniformly across step kinds). The `LlmMapDispatcher`
/// aborts the step the moment its running item-token sum exceeds this.
pub const PER_STEP_TOKEN_CAP: u64 = 2_000_000;

/// Per-run cumulative output + artifact byte cap (plan §4.5 + §10).
/// Enforced by the runner after each step's outputs + artifacts are
/// written; the run aborts (Failed) once the cumulative crosses it.
/// `artifact_io::PER_RUN_ARTIFACT_CAP_BYTES` is the same 100 MiB value
/// (declared there but historically never enforced — audit gap 6).
pub const PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES: u64 = artifact_io::PER_RUN_ARTIFACT_CAP_BYTES;

/// Pure cap-check applied by the runner after each step completes.
/// Returns `Err(reason)` when ANY of the per-step token cap, per-run
/// token cap, or per-run output+artifact byte cap is exceeded. Factored
/// out so the cap logic is unit-testable without driving a full run
/// (plan §4.5 + §10 / audit gaps 5 + 6).
pub(crate) fn check_step_caps(
    step_id: &str,
    step_tokens_used: u64,
    run_total_tokens: u64,
    run_total_output_bytes: u64,
) -> Result<(), String> {
    if step_tokens_used > PER_STEP_TOKEN_CAP {
        return Err(format!(
            "per-step token cap {PER_STEP_TOKEN_CAP} exceeded \
             ({step_tokens_used} used in step '{step_id}')"
        ));
    }
    if run_total_tokens > PER_RUN_TOKEN_CAP {
        return Err(format!(
            "per-run token cap {PER_RUN_TOKEN_CAP} exceeded ({run_total_tokens} used)"
        ));
    }
    if run_total_output_bytes > PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES {
        return Err(format!(
            "per-run output+artifact byte cap {PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES} \
             exceeded ({run_total_output_bytes} written)"
        ));
    }
    Ok(())
}

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
    model_max_tokens: u32,
    sandbox_flavor: Option<String>,
    is_dev: bool,
    mocks: HashMap<String, Value>,
    force_mocks: bool,
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
        model_max_tokens,
        sandbox_flavor,
        total_tokens: 0,
        total_output_bytes: 0,
        is_dev,
        // Mocks only honored for dev workflows OR test runs (force_mocks).
        // The /run handler already gates the dev case (403 when mocks present
        // on a published workflow); the /test handler sets force_mocks so a
        // published workflow's tests/ fixtures still run with mocks (the
        // sanctioned mock context — plan §3). Belt-and-suspenders: drop the
        // mocks here if neither condition holds.
        mocks: if is_dev || force_mocks {
            mocks
        } else {
            HashMap::new()
        },
        force_mocks,
        // Defaults; spawn_run overrides after preflight. The /test path keeps
        // these off (no artifact persistence; the workflow's own log levels).
        persist_artifacts: false,
        force_log_capture: false,
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

    // Liveness heartbeat: bump `updated_at` every HEARTBEAT_INTERVAL so the
    // workflow_mcp no-progress guard sees a live runner even during a long
    // single step (elicit / sandbox) that emits no step transitions. A
    // crashed runner task can't tick this, so the guard still catches it.
    let hb_pool = pool.clone();
    let heartbeat = tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the immediate first tick (mark_running already set updated_at).
        tick.tick().await;
        loop {
            tick.tick().await;
            // M-4: a TRANSIENT DB error must NOT stop the heartbeat — the
            // no-progress guard relies on this signal, and stopping early
            // would let it false-kill a healthy run. A terminal/gone run
            // makes the guarded UPDATE a harmless no-op; the AbortOnDrop
            // guard below is what actually stops the loop on run exit.
            let _ = repository::heartbeat(&hb_pool, run_id).await;
        }
    });
    // #1a: abort the heartbeat on EVERY exit of run_workflow — including a
    // panic unwinding past the wall-clock await (timeout doesn't catch
    // panics, and run_workflow is a detached task). A bare abort() after the
    // await would be skipped on panic, leaking a heartbeat that keeps
    // updated_at fresh and defeats the very no-progress guard it serves.
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _heartbeat_guard = AbortOnDrop(heartbeat);

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

    // Cleanup the EPHEMERAL scratch (the bundle copy + staged stdin) to
    // reclaim disk, but KEEP outputs/ artifacts/ logs/ so the per-step
    // output / artifact / log REST endpoints AND workflow_mcp resources can
    // be read AFTER the run reaches a terminal status — that's the whole
    // point of those surfaces (and the LLM may resources/read immediately
    // after the tool call returns). The staged dir is GC'd in full by the
    // startup sweep on the next restart. (The plan's "rm -rf the whole run
    // dir on terminal" conflicted with its own results-readable-after-
    // completion contract; results must outlive the terminal transition.)
    for sub in ["scripts", "references", "prompts", "inputs"] {
        let _ = tokio::fs::remove_dir_all(ctx.sandbox_workspace.join(sub)).await;
    }
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

        // Mock short-circuit. Honor a per-run `mocks[step.id]` from the
        // /run body OR a `StepDef.mock` baked into the workflow. Skips real
        // dispatch entirely — no LLM tokens, no sandbox spawn.
        // Gated on `is_dev` (the /run handler rejects mocks for published
        // workflows, and RunContext drops them when !is_dev) OR `force_mocks`
        // (set ONLY by the /test handler — the sanctioned mock context that
        // lets a published workflow's tests/ fixtures run with mocks).
        // See plan §1 + §3 + B4 audit.
        let mock_value: Option<Value> = if ctx.is_dev || ctx.force_mocks {
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
                StepConfig::Tool { .. } => Box::new(ToolDispatcher::new()),
            };
            tokio::select! {
                r = dispatcher.dispatch(step, ctx, handle.clone(), emit.clone()) => r,
                _ = handle.await_cancel() => StepResult::Cancelled,
            }
        };

        match result {
            StepResult::Completed { output, parsed_as, tokens_used, ms_elapsed } => {
                // Persist meta (already wrote the file). Tally output bytes
                // toward the per-run output+artifact cap.
                if let Some(meta) = ctx.step_outputs.get(&step.id).cloned() {
                    ctx.total_output_bytes =
                        ctx.total_output_bytes.saturating_add(meta.size_bytes);
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
                // Collect step artifacts (sandbox steps only). M3: the
                // collector now enforces the per-run cap PRE-WRITE and
                // returns Err when an artifact would cross it — fail the
                // run instead of swallowing it (`unwrap_or_default`).
                if matches!(step.config, StepConfig::Sandbox { .. }) {
                    let artifacts = match artifact_io::collect_step_artifacts(ctx, step) {
                        Ok(a) => a,
                        Err(e) => {
                            return RunInnerOutcome::Failed {
                                error: format!("step '{}' artifact cap: {e}", step.id),
                                failed_at_step: Some(step.id.clone()),
                            };
                        }
                    };
                    if !artifacts.is_empty() {
                        // Tally artifact bytes toward the per-run cap.
                        let art_bytes: u64 = artifacts.iter().map(|a| a.size_bytes).sum();
                        ctx.total_output_bytes =
                            ctx.total_output_bytes.saturating_add(art_bytes);
                        let json = serde_json::to_value(&artifacts).unwrap_or(Value::Null);
                        let _ = repository::persist_step_artifacts(
                            pool,
                            ctx.run_id,
                            &step.id,
                            &json,
                        )
                        .await;

                        // A3: durable persistence. When launched standalone
                        // (REST /run → persist_artifacts=true) copy each collected
                        // artifact into the user file store so it survives the
                        // staging-dir GC + shows in Files (created_by="workflow",
                        // linked to the run). MCP-tool-call runs set
                        // persist_artifacts=false — the chat extension persists
                        // their resource_links instead (no double-save).
                        if ctx.persist_artifacts {
                            for art in &artifacts {
                                match tokio::fs::read(&art.host_path).await {
                                    Ok(bytes) => {
                                        if let Err(e) =
                                            crate::modules::file::ingest::ingest_bytes(
                                                ctx.user_id,
                                                &bytes,
                                                &art.filename,
                                                Some(art.mime_type.clone()),
                                                "workflow",
                                                None,
                                                Some(ctx.run_id),
                                            )
                                            .await
                                        {
                                            tracing::warn!(
                                                "workflow: persist artifact '{}' to file store failed: {e}",
                                                art.filename
                                            );
                                        }
                                    }
                                    Err(e) => tracing::warn!(
                                        "workflow: read artifact {} for persistence failed: {e}",
                                        art.host_path.display()
                                    ),
                                }
                            }
                        }
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
                // Per-step token cap (the LlmMapDispatcher also self-aborts
                // mid-fan-out — this is the uniform backstop), per-run token
                // cap, and per-run output+artifact byte cap (audit gaps 5+6).
                if let Err(reason) = check_step_caps(
                    &step.id,
                    tokens_used,
                    ctx.total_tokens,
                    ctx.total_output_bytes,
                ) {
                    return RunInnerOutcome::Failed {
                        error: reason,
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
        // The render returns a string; preview cap. L1: char-safe
        // truncation — a byte slice `&rendered[..500]` panics if 500
        // lands mid-UTF-8-codepoint (LLM output is arbitrary text),
        // crashing the runner task. Take 500 CHARS instead.
        let truncated = if rendered.chars().count() > 500 {
            format!("{}…", rendered.chars().take(500).collect::<String>())
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

/// Resolve `outputs[]` to their FULL values (not the 500-char previews
/// `resolve_outputs` writes into `final_output_json`). Used by the
/// `POST /api/workflows/{id}/test` runner (B6) so fixture assertions
/// (`min_length`, `matches_schema`, `equals`) see the real output. Each
/// output's `from` template renders to a string; if that string parses
/// as JSON we keep the parsed Value (so array/object schema assertions
/// work), else we keep the string.
pub async fn resolve_outputs_full(
    ctx: &mut RunContext,
    outputs: &[OutputDef],
) -> Result<serde_json::Map<String, Value>, AppError> {
    let mut map = serde_json::Map::new();
    for o in outputs {
        let rendered = crate::modules::workflow::template::render(&o.from, ctx)
            .map_err(|e| AppError::internal_error(format!("output '{}': {e}", o.name)))?;
        let value = serde_json::from_str::<Value>(&rendered)
            .unwrap_or_else(|_| Value::String(rendered));
        map.insert(o.name.clone(), value);
    }
    Ok(map)
}

/// Outcome of a synchronous test run (B6).
pub struct TestRunOutcome {
    pub run_id: Uuid,
    pub status: WorkflowRunStatus,
    pub error: Option<String>,
    /// Full resolved outputs (only populated on `Completed`).
    pub outputs: serde_json::Map<String, Value>,
}

/// Run a workflow to terminal status IN-PROCESS (no fire-and-forget
/// spawn) and return the FULL resolved outputs. Powers
/// `POST /api/workflows/{id}/test` (B6).
///
/// `force_mocks` is threaded into the RunContext so a published
/// (`is_dev = false`) workflow's `tests/` fixtures still honor mocks —
/// the sanctioned mock context (plan §3). The /run endpoint's is_dev
/// gate is untouched: only the test handler passes `force_mocks: true`.
///
/// The caller owns the `workflow_runs` row insert (with
/// `run_kind = 'test'`) — this fn just drives execution + output
/// resolution and cleans the staged dir afterward.
#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    workflow: &crate::modules::workflow::models::Workflow,
    workflow_def: &WorkflowDef,
    inputs: Value,
    mocks: HashMap<String, Value>,
    model_id: Uuid,
    model_name: String,
    provider: Arc<ai_providers::Provider>,
) -> Result<TestRunOutcome, AppError> {
    let sandbox_flavor = workflow_def.sandbox.as_ref().map(|s| s.flavor.clone());
    let _handle = registry::register(run_id);
    let workspace_root = workflow_workspace_root();
    let mut ctx = preflight(
        pool,
        run_id,
        user_id,
        conversation_id,
        workflow.id,
        inputs,
        workflow_def,
        PathBuf::from(&workflow.extracted_path),
        workspace_root,
        model_id,
        model_name,
        // /test runs mock every llm step, so the request max_tokens is never
        // sent — the chat-path default is fine here.
        8192,
        sandbox_flavor,
        workflow.is_dev,
        mocks,
        true, // force_mocks: sanctioned mock context for test runs
    )
    .await?;

    let handle = match registry::get(run_id) {
        Some(h) => h,
        None => registry::register(run_id),
    };
    let emit: Arc<dyn ProgressEmitter> = Arc::new(PerRunEmitter { run_id });

    let outcome = tokio::time::timeout(
        RUN_WALL_CLOCK,
        run_inner(pool, &mut ctx, workflow_def, provider, handle.clone(), emit.clone()),
    )
    .await;

    let result = match outcome {
        Ok(RunInnerOutcome::Completed { .. }) => {
            let _ = repository::mark_status(pool, run_id, WorkflowRunStatus::Completed, None).await;
            let outputs = resolve_outputs_full(&mut ctx, &workflow_def.outputs)
                .await
                .unwrap_or_default();
            TestRunOutcome {
                run_id,
                status: WorkflowRunStatus::Completed,
                error: None,
                outputs,
            }
        }
        Ok(RunInnerOutcome::Failed { error, .. }) => {
            let _ =
                repository::mark_status(pool, run_id, WorkflowRunStatus::Failed, Some(&error)).await;
            TestRunOutcome {
                run_id,
                status: WorkflowRunStatus::Failed,
                error: Some(error),
                outputs: Default::default(),
            }
        }
        Ok(RunInnerOutcome::Cancelled { .. }) => {
            let _ = repository::mark_status(
                pool,
                run_id,
                WorkflowRunStatus::Cancelled,
                Some("cancelled"),
            )
            .await;
            TestRunOutcome {
                run_id,
                status: WorkflowRunStatus::Cancelled,
                error: Some("cancelled".into()),
                outputs: Default::default(),
            }
        }
        Err(_) => {
            let err = "workflow test runner wall-clock timeout (30 min)".to_string();
            let _ =
                repository::mark_status(pool, run_id, WorkflowRunStatus::Failed, Some(&err)).await;
            TestRunOutcome {
                run_id,
                status: WorkflowRunStatus::Failed,
                error: Some(err),
                outputs: Default::default(),
            }
        }
    };

    // Cleanup the staged dir + registry entry.
    let _ = tokio::fs::remove_dir_all(&ctx.sandbox_workspace).await;
    registry::unregister(run_id);

    Ok(result)
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
    opts: SpawnRunOpts,
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

    // Resolve the model: an explicit `model_id` (standalone run, access-checked)
    // wins; otherwise snapshot the conversation's model. The model max output
    // (fallback 8192) is used for llm requests — same as the chat path's
    // apply_model_params (the per-call cost cap is enforced post-call, NOT here:
    // hardcoding 50k exceeds many models' output limits and the provider rejects).
    let (model_id, model_name, model_max_tokens) =
        resolve_run_model(user_id, opts.model_id, conversation_id).await?;

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
            invocation_source: opts.invocation_source.to_string(),
            inputs_json: inputs.clone(),
        },
    )
    .await?;

    let _handle = registry::register(row.id);

    let workspace_root = workflow_workspace_root();
    let mut ctx = preflight(
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
        model_max_tokens,
        sandbox_flavor,
        workflow.is_dev,
        mocks,
        false, // force_mocks: normal /run path uses the is_dev gate
    )
    .await?;
    // A1/A3/A7: thread the invocation-path options the runner consumes later.
    ctx.persist_artifacts = opts.persist_artifacts;
    ctx.force_log_capture = opts.force_log_capture;

    let (provider, _name, _mid, _pid, _params) =
        crate::modules::chat::core::ai_provider::create_provider_from_model_id(model_id, user_id)
            .await?;

    let pool_for_task = pool.clone();
    tokio::spawn(async move {
        run_workflow(pool_for_task, ctx, workflow_def, provider).await;
    });

    Ok(row.id)
}

/// Options threaded into a run from its invocation path (REST `/run` vs the
/// `workflow_mcp` tool call).
pub struct SpawnRunOpts {
    /// Explicit model for a standalone run; validated + access-checked. `None`
    /// → snapshot the model from `conversation_id`.
    pub model_id: Option<Uuid>,
    /// `"manual"` (workflow page) or `"conversation"` (LLM tool call) — recorded
    /// on the run for the history view.
    pub invocation_source: &'static str,
    /// Persist declared artifacts + tool-result files to the file store on
    /// completion. `true` on REST `/run`; `false` on `workflow_mcp` (the chat
    /// extension persists those instead).
    pub persist_artifacts: bool,
    /// Force full per-step log capture for this run (the per-run debug toggle).
    pub force_log_capture: bool,
}

/// Resolve the model for a run. Precedence: an explicit `model_id` (validated +
/// access-checked against the user's providers) wins; otherwise snapshot the
/// conversation's model; otherwise error. Returns `(model_id, name, max_tokens)`.
async fn resolve_run_model(
    user_id: Uuid,
    model_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
) -> Result<(Uuid, String, u32), AppError> {
    let model = if let Some(mid) = model_id {
        let model = crate::core::Repos
            .llm_model
            .get_by_id(mid)
            .await?
            .ok_or_else(|| AppError::not_found("Model"))?;
        if !model.enabled {
            return Err(AppError::bad_request(
                "MODEL_DISABLED",
                "this model is currently disabled and cannot be used",
            ));
        }
        // The run handler is only gated on WorkflowsExecute; without this a
        // user could name any model_id and bypass provider access control.
        let has_access = crate::core::Repos
            .user_group_llm_provider
            .user_has_access_to_provider(user_id, model.provider_id)
            .await
            .map_err(AppError::from)?;
        if !has_access {
            return Err(AppError::forbidden(
                "ACCESS_DENIED",
                "you do not have access to this model",
            ));
        }
        model
    } else if let Some(conv_id) = conversation_id {
        let conv = crate::core::Repos
            .chat
            .core
            .get_conversation(conv_id, user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Conversation"))?;
        let mid = conv.model_id.ok_or_else(|| {
            AppError::bad_request(
                "WORKFLOW_CONVERSATION_NO_MODEL",
                "conversation has no model set; cannot snapshot for workflow run",
            )
        })?;
        crate::core::Repos
            .llm_model
            .get_by_id(mid)
            .await?
            .ok_or_else(|| AppError::not_found("Model"))?
    } else {
        return Err(AppError::bad_request(
            "WORKFLOW_NO_MODEL_SOURCE",
            "provide a model_id or a conversation_id to run a workflow",
        ));
    };
    let model_name = model.name.clone();
    let model_max_tokens = model
        .parameters
        .max_tokens
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .unwrap_or(8192);
    Ok((model.id, model_name, model_max_tokens))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_pass_under_limits() {
        assert!(check_step_caps("s", 10_000, 100_000, 1_024).is_ok());
        // Exactly at the caps is allowed (the runner uses strict `>`).
        assert!(
            check_step_caps(
                "s",
                PER_STEP_TOKEN_CAP,
                PER_RUN_TOKEN_CAP,
                PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES,
            )
            .is_ok()
        );
    }

    #[test]
    fn per_step_token_cap_trips() {
        let err = check_step_caps("gen", PER_STEP_TOKEN_CAP + 1, 0, 0)
            .expect_err("per-step cap should trip");
        assert!(err.contains("per-step token cap"), "got: {err}");
    }

    #[test]
    fn per_run_token_cap_trips() {
        let err = check_step_caps("gen", 1, PER_RUN_TOKEN_CAP + 1, 0)
            .expect_err("per-run token cap should trip");
        assert!(err.contains("per-run token cap"), "got: {err}");
    }

    #[test]
    fn per_run_output_byte_cap_trips() {
        let err =
            check_step_caps("gen", 1, 1, PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES + 1)
                .expect_err("per-run byte cap should trip");
        assert!(err.contains("output+artifact byte cap"), "got: {err}");
    }

    #[test]
    fn cap_constant_values_match_plan() {
        // Plan §4.5 + §10: per-step 2M, per-run 5M, 100 MiB output+artifact.
        assert_eq!(PER_STEP_TOKEN_CAP, 2_000_000);
        assert_eq!(PER_RUN_TOKEN_CAP, 5_000_000);
        assert_eq!(PER_RUN_OUTPUT_ARTIFACT_CAP_BYTES, 100 * 1024 * 1024);
    }
}
