//! Step dispatchers (plan §4.5 + §4.6 + §4.7).
//!
//! Four impls of the single `StepDispatcher` trait — one per step kind.
//! Each one:
//! 1. resolves templates against `ctx`,
//! 2. wraps the blocking await in `tokio::select! { _, cancel }`,
//! 3. writes the step output FILE via `file_io::write_step_output`,
//! 4. updates `ctx.step_outputs` with the meta.
//!
//! Persistence of the per-step metadata into `workflow_runs` happens
//! in the runner (one place, transactional with status updates).

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use ai_providers::{
    ChatMessage, ChatRequest, ContentBlock as AiBlock, ContentBlockDelta, Provider,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::code_sandbox::tools::execute::execute_command_with_mounts;
use crate::modules::code_sandbox::types::SandboxContext;
use crate::modules::code_sandbox::workflow_staging::{StageMode, StagedMount};
use crate::modules::workflow::events::{
    ProgressEmitter, SSEElicitationRequiredData, SSEElicitationResolvedData,
    SSEStepItemProgressData, SSEWorkflowRunEvent,
};
use crate::modules::workflow::file_io;
use crate::modules::workflow::log_io;
use crate::modules::workflow::registry;
use crate::modules::workflow::types::{
    ItemProgress, OutputMeta, ParsedAs, RunContext, StepKindTag, StepResult,
};
use crate::modules::workflow::validate::{
    LogCapture, OnError, OutputFormat, StepConfig, StepDef,
};

/// Per-call LLM token cap (plan §4.5).
pub const PER_CALL_TOKEN_CAP: u64 = 50_000;

/// Per-step token cap (plan §4.5 + §10). Re-exported from the runner so
/// the `LlmMapDispatcher` can abort the step the moment the running sum
/// of its item tokens crosses the cap (rather than only at the runner's
/// post-step backstop). Aggregate across all `llm_map` items.
pub use crate::modules::workflow::runner::PER_STEP_TOKEN_CAP;

#[async_trait]
pub trait StepDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult;
}

// ============================================================
// LLM dispatcher (§4.5)
// ============================================================

pub struct LlmDispatcher {
    pub provider: Arc<Provider>,
}

impl LlmDispatcher {
    pub fn new(provider: Arc<Provider>) -> Self {
        Self { provider }
    }
}

/// Resolve prompt from inline `prompt:` or `prompt_file:`. Templates
/// are rendered against `ctx`.
async fn resolve_prompt(
    step: &StepDef,
    ctx: &RunContext,
    prompt: &Option<String>,
    prompt_file: &Option<String>,
) -> Result<String, String> {
    let raw = load_raw_prompt(step, ctx, prompt, prompt_file).await?;
    crate::modules::workflow::template::render(&raw, ctx).map_err(|e| e.to_string())
}

/// Load the RAW (un-rendered) prompt from inline `prompt:` or
/// `prompt_file:`. Used by `llm_map`, whose per-item prompt contains the
/// `{{ <item_var> }}` binding that does NOT exist in `ctx` — so it must be
/// rendered per-item via `render_with_bindings` (H4), never pre-rendered
/// against `ctx` alone.
async fn load_raw_prompt(
    step: &StepDef,
    ctx: &RunContext,
    prompt: &Option<String>,
    prompt_file: &Option<String>,
) -> Result<String, String> {
    match (prompt, prompt_file) {
        (Some(p), None) => Ok(p.clone()),
        (None, Some(rel)) => {
            let path = ctx.extracted_path.join(rel);
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("read prompt_file '{rel}': {e}"))
        }
        _ => Err(format!("step '{}' has invalid prompt config", step.id)),
    }
}

#[async_trait]
impl StepDispatcher for LlmDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let started = Instant::now();
        let _ = emit;

        let (prompt, prompt_file, output_format) = match &step.config {
            StepConfig::Llm {
                prompt,
                prompt_file,
                output_format,
                ..
            } => (prompt.clone(), prompt_file.clone(), *output_format),
            _ => {
                return StepResult::Failed {
                    error: "LlmDispatcher called on non-llm step".into(),
                    tokens_used: 0,
                };
            }
        };

        let rendered = match resolve_prompt(step, ctx, &prompt, &prompt_file).await {
            Ok(r) => r,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("prompt render: {e}"),
                    tokens_used: 0,
                };
            }
        };

        // Capture rendered prompt to logs (gated).
        let _ = log_io::write_text_log(ctx, &step.id, "prompt", &rendered, step.log).await;

        let req = ChatRequest {
            model: ctx.model_name.clone(),
            messages: vec![ChatMessage::user(rendered.clone())],
            max_tokens: Some(PER_CALL_TOKEN_CAP as u32),
            ..Default::default()
        };

        let result = run_llm_call(&self.provider, req, cancel.clone()).await;
        let (text, tokens) = match result {
            LlmCallOutcome::Cancelled => return StepResult::Cancelled,
            LlmCallOutcome::Failed(e) => {
                return StepResult::Failed {
                    error: e,
                    tokens_used: 0,
                };
            }
            LlmCallOutcome::Ok { text, tokens } => (text, tokens),
        };

        // Capture raw LLM response (gated).
        let _ = log_io::write_text_log(ctx, &step.id, "raw_output", &text, step.log).await;

        let (value, parsed_as) = match output_format {
            OutputFormat::Text => (Value::String(text.clone()), ParsedAs::Text),
            OutputFormat::Json => match serde_json::from_str::<Value>(&text) {
                Ok(v) => (v, ParsedAs::Json),
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("expected JSON output, parse failed: {e}"),
                        tokens_used: tokens,
                    };
                }
            },
        };

        // Write output file + register meta on ctx.
        let meta_res = file_io::write_step_output(
            ctx,
            &step.id,
            &value,
            parsed_as,
            StepKindTag::Llm,
        )
        .await;
        let meta = match meta_res {
            Ok(m) => m,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("persist step output: {e}"),
                    tokens_used: tokens,
                };
            }
        };
        ctx.step_outputs.insert(step.id.clone(), meta);
        ctx.total_tokens += tokens;

        StepResult::Completed {
            output: value,
            parsed_as,
            tokens_used: tokens,
            ms_elapsed: started.elapsed().as_millis() as u64,
        }
    }
}

enum LlmCallOutcome {
    Ok { text: String, tokens: u64 },
    Failed(String),
    Cancelled,
}

/// Cancel-aware streaming accumulation (plan §4.5).
async fn run_llm_call(
    provider: &Arc<Provider>,
    req: ChatRequest,
    cancel: Arc<registry::RunHandle>,
) -> LlmCallOutcome {
    let mut stream = match provider.chat_stream(req).await {
        Ok(s) => s,
        Err(e) => return LlmCallOutcome::Failed(format!("provider.chat_stream: {e}")),
    };
    let mut text = String::new();
    let mut tokens: u64 = 0;
    loop {
        tokio::select! {
            chunk = stream.next() => match chunk {
                Some(Ok(c)) => {
                    for d in c.content {
                        if let ContentBlockDelta::TextDelta { delta, .. } = d {
                            text.push_str(&delta);
                        }
                    }
                    if let Some(u) = c.usage {
                        tokens = u.total_tokens as u64;
                    }
                }
                Some(Err(e)) => return LlmCallOutcome::Failed(format!("stream chunk: {e}")),
                None => break,
            },
            _ = cancel.await_cancel() => return LlmCallOutcome::Cancelled,
        }
    }
    LlmCallOutcome::Ok { text, tokens }
}

// ============================================================
// LLM map dispatcher (§4.5) — fan-out
// ============================================================

pub struct LlmMapDispatcher {
    pub provider: Arc<Provider>,
}

impl LlmMapDispatcher {
    pub fn new(provider: Arc<Provider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl StepDispatcher for LlmMapDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let started = Instant::now();
        let (prompt, prompt_file, for_each, item_var, output_format, max_parallel, on_error, max_retries) =
            match &step.config {
                StepConfig::LlmMap {
                    prompt,
                    prompt_file,
                    for_each,
                    item_var,
                    output_format,
                    max_parallel,
                    on_error,
                    max_retries,
                    ..
                } => (
                    prompt.clone(),
                    prompt_file.clone(),
                    for_each.clone(),
                    item_var.clone(),
                    *output_format,
                    *max_parallel,
                    *on_error,
                    *max_retries,
                ),
                _ => {
                    return StepResult::Failed {
                        error: "LlmMapDispatcher called on non-llm_map step".into(),
                        tokens_used: 0,
                    };
                }
            };

        // Resolve for_each → array.
        let for_each_rendered =
            match crate::modules::workflow::template::render(&for_each, ctx) {
                Ok(s) => s,
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("for_each render: {e}"),
                        tokens_used: 0,
                    };
                }
            };
        let items: Vec<Value> = match serde_json::from_str::<Value>(&for_each_rendered) {
            Ok(Value::Array(a)) => a,
            Ok(other) => {
                return StepResult::Failed {
                    error: format!(
                        "llm_map for_each must resolve to array; got {}",
                        other_type_name(&other)
                    ),
                    tokens_used: 0,
                };
            }
            Err(e) => {
                return StepResult::Failed {
                    error: format!("llm_map for_each parse: {e}"),
                    tokens_used: 0,
                };
            }
        };

        let total = items.len() as u32;
        let parallel = max_parallel.min(crate::modules::workflow::validate::MAX_PARALLEL_HARD_CAP);
        let sem = Arc::new(Semaphore::new(parallel as usize));

        // Per-item prompts: render with `{{ <item_var> }}` bound through the
        // REAL template engine (H4). We load the RAW prompt (NOT pre-rendered
        // against ctx — it carries the `{{ <item_var> }}` binding) then render
        // it once per item on the main task (which holds `&ctx`, so ctx refs
        // like `{{ inputs.x }}` / `{{ s.output }}` resolve too), binding the
        // item as a resolvable variable so `{{ <item_var>.field }}` /
        // `{{ <item_var>[N] }}` work when items are objects/arrays. The
        // finished string is moved into the spawned task.
        let raw_prompt = match load_raw_prompt(step, ctx, &prompt, &prompt_file).await {
            Ok(p) => p,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("prompt load: {e}"),
                    tokens_used: 0,
                };
            }
        };
        let mut per_item_prompts: Vec<String> = Vec::with_capacity(items.len());
        for (idx, item) in items.iter().enumerate() {
            let mut binding = std::collections::HashMap::new();
            binding.insert(item_var.clone(), item.clone());
            match crate::modules::workflow::template::render_with_bindings(
                &raw_prompt, ctx, &binding,
            ) {
                Ok(p) => per_item_prompts.push(p),
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("item {idx} prompt render: {e}"),
                        tokens_used: 0,
                    };
                }
            }
        }

        // For each item we'll spawn a future and collect results in input order.
        let mut handles: Vec<tokio::task::JoinHandle<(usize, Result<Value, String>, u64)>> =
            Vec::with_capacity(items.len());

        // Initial progress event.
        ctx.step_item_progress.insert(
            step.id.clone(),
            ItemProgress {
                completed: 0,
                total,
                failed: 0,
                skipped: 0,
                tokens_so_far: 0,
            },
        );
        emit.emit(SSEWorkflowRunEvent::StepItemProgress(SSEStepItemProgressData {
            run_id: ctx.run_id,
            step_id: step.id.clone(),
            progress: ctx.step_item_progress[&step.id].clone(),
        }));

        for (idx, prompt) in per_item_prompts.into_iter().enumerate() {
            let provider = self.provider.clone();
            let cancel_clone = cancel.clone();
            let sem_clone = sem.clone();
            let model = ctx.model_name.clone();

            let h = tokio::spawn(async move {
                let _permit = match sem_clone.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => return (idx, Err("semaphore closed".into()), 0),
                };
                if cancel_clone.is_cancelled() {
                    return (idx, Err("cancelled".into()), 0);
                }

                let mut attempts: u32 = 0;
                let max_attempts = if on_error == OnError::Retry {
                    max_retries.saturating_add(1).max(1)
                } else {
                    1
                };
                let mut last_err = String::new();
                let mut total_tokens: u64 = 0;
                loop {
                    attempts += 1;
                    let req = ChatRequest {
                        model: model.clone(),
                        messages: vec![ChatMessage::user(prompt.clone())],
                        max_tokens: Some(PER_CALL_TOKEN_CAP as u32),
                        ..Default::default()
                    };
                    match run_llm_call(&provider, req, cancel_clone.clone()).await {
                        LlmCallOutcome::Ok { text, tokens } => {
                            total_tokens += tokens;
                            let parsed: Value = match output_format {
                                OutputFormat::Text => Value::String(text),
                                OutputFormat::Json => match serde_json::from_str(&text) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        last_err = format!("JSON parse: {e}");
                                        if attempts < max_attempts {
                                            tokio::time::sleep(retry_backoff(attempts)).await;
                                            continue;
                                        }
                                        break;
                                    }
                                },
                            };
                            return (idx, Ok(parsed), total_tokens);
                        }
                        LlmCallOutcome::Cancelled => {
                            return (idx, Err("cancelled".into()), total_tokens);
                        }
                        LlmCallOutcome::Failed(e) => {
                            last_err = e;
                            if attempts < max_attempts {
                                tokio::time::sleep(retry_backoff(attempts)).await;
                                continue;
                            }
                            break;
                        }
                    }
                }
                (idx, Err(last_err), total_tokens)
            });
            handles.push(h);
        }

        // Drain.
        let mut results: Vec<Option<Value>> = vec![None; items.len()];
        let mut failed = 0u32;
        let mut skipped = 0u32;
        let mut completed = 0u32;
        let mut total_tokens: u64 = 0;
        let mut any_failed_fatal: Option<String> = None;
        for h in handles {
            let res = match h.await {
                Ok(r) => r,
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("llm_map task join: {e}"),
                        tokens_used: total_tokens,
                    };
                }
            };
            if cancel.is_cancelled() {
                // L3: snapshot the partial item progress at cancel so the
                // run row + any live SSE client reflect what completed before
                // the cancel (the cancelled count is derivable as
                // total - completed - failed - skipped).
                let progress = ItemProgress {
                    completed,
                    total,
                    failed,
                    skipped,
                    tokens_so_far: total_tokens,
                };
                ctx.step_item_progress
                    .insert(step.id.clone(), progress.clone());
                emit.emit(SSEWorkflowRunEvent::StepItemProgress(SSEStepItemProgressData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    progress,
                }));
                return StepResult::Cancelled;
            }
            let (idx, outcome, item_tokens) = res;
            total_tokens += item_tokens;
            // Per-step token cap (plan §4.5 + §10): abort the whole step the
            // moment the aggregate across processed items exceeds 2M. We do
            // NOT spawn-cancel the remaining in-flight items here (they hold
            // their own per-call 50k cap + share the run's cancel handle and
            // wall-clock); failing the step propagates to RunFailed.
            if total_tokens > PER_STEP_TOKEN_CAP {
                return StepResult::Failed {
                    error: format!(
                        "per-step token cap {PER_STEP_TOKEN_CAP} exceeded \
                         ({total_tokens} used across llm_map items)"
                    ),
                    tokens_used: total_tokens,
                };
            }
            match outcome {
                Ok(v) => {
                    results[idx] = Some(v);
                    completed += 1;
                }
                Err(err) => {
                    let (fatal, result_value) = classify_item_error(on_error);
                    if fatal {
                        // Fail / Retry-exhausted → abort the step (fail-shape).
                        failed += 1;
                        any_failed_fatal.get_or_insert(format!("item {idx}: {err}"));
                    } else {
                        // Skip → record Null + keep going.
                        skipped += 1;
                        results[idx] = result_value;
                    }
                }
            }
            // Emit per-item progress update.
            let progress = ItemProgress {
                completed,
                total,
                failed,
                skipped,
                tokens_so_far: total_tokens,
            };
            ctx.step_item_progress
                .insert(step.id.clone(), progress.clone());
            emit.emit(SSEWorkflowRunEvent::StepItemProgress(SSEStepItemProgressData {
                run_id: ctx.run_id,
                step_id: step.id.clone(),
                progress,
            }));
        }

        if let Some(err) = any_failed_fatal {
            return StepResult::Failed {
                error: err,
                tokens_used: total_tokens,
            };
        }

        // Assemble output array in order.
        let arr: Vec<Value> = results.into_iter().map(|o| o.unwrap_or(Value::Null)).collect();
        let value = Value::Array(arr);

        let meta = match file_io::write_step_output(
            ctx,
            &step.id,
            &value,
            ParsedAs::Json,
            StepKindTag::LlmMap,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("persist step output: {e}"),
                    tokens_used: total_tokens,
                };
            }
        };
        ctx.step_outputs.insert(step.id.clone(), meta);
        ctx.total_tokens += total_tokens;

        StepResult::Completed {
            output: value,
            parsed_as: ParsedAs::Json,
            tokens_used: total_tokens,
            ms_elapsed: started.elapsed().as_millis() as u64,
        }
    }
}

fn retry_backoff(attempt: u32) -> std::time::Duration {
    // 250ms, 500ms, 1s, 2s, 4s (capped at 8s).
    let base = 250u64 * (1u64 << attempt.min(6));
    std::time::Duration::from_millis(base.min(8_000))
}

/// Per-item on-error outcome classification (pure). Returns
/// `(is_fatal, result_value)`: `is_fatal=true` aborts the whole llm_map
/// step (Fail / Retry-exhausted), `false` records the item as skipped
/// with a Null result. Factored out of the inline `match` so the
/// branch semantics are unit-testable without spawning real item calls.
fn classify_item_error(on_error: OnError) -> (bool, Option<Value>) {
    match on_error {
        // Fail and Retry-exhausted are both fatal for the step.
        OnError::Fail | OnError::Retry => (true, None),
        // Skip records the item as Null and keeps going.
        OnError::Skip => (false, Some(Value::Null)),
    }
}

fn other_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ============================================================
// Sandbox dispatcher (§4.5)
// ============================================================

pub struct SandboxDispatcher;

impl SandboxDispatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SandboxDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StepDispatcher for SandboxDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let _ = emit;
        let started = Instant::now();

        let (run, stdin, _timeout_ms) = match &step.config {
            StepConfig::Sandbox {
                run,
                stdin,
                timeout_ms,
            } => (run.clone(), stdin.clone(), *timeout_ms),
            _ => {
                return StepResult::Failed {
                    error: "SandboxDispatcher called on non-sandbox step".into(),
                    tokens_used: 0,
                };
            }
        };

        let flavor = match ctx.sandbox_flavor.clone() {
            Some(f) => f,
            None => {
                return StepResult::Failed {
                    error: "sandbox step but workflow has no sandbox.flavor".into(),
                    tokens_used: 0,
                };
            }
        };

        // Render run + stdin templates.
        let run_rendered = match crate::modules::workflow::template::render(&run, ctx) {
            Ok(s) => s,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("run: render failed: {e}"),
                    tokens_used: 0,
                };
            }
        };

        // Stage stdin file if set.
        let stdin_path_sandbox = if let Some(stdin_tpl) = &stdin {
            let text =
                match crate::modules::workflow::template::render(stdin_tpl, ctx) {
                    Ok(s) => s,
                    Err(e) => {
                        return StepResult::Failed {
                            error: format!("stdin: render failed: {e}"),
                            tokens_used: 0,
                        };
                    }
                };
            let inputs_dir = &ctx.inputs_dir;
            if let Err(e) = tokio::fs::create_dir_all(inputs_dir).await {
                return StepResult::Failed {
                    error: format!("mkdir inputs dir: {e}"),
                    tokens_used: 0,
                };
            }
            let host_path = inputs_dir.join(format!("{}.txt", step.id));
            if let Err(e) = tokio::fs::write(&host_path, text.as_bytes()).await {
                return StepResult::Failed {
                    error: format!("write stdin: {e}"),
                    tokens_used: 0,
                };
            }
            Some(format!("inputs/{}.txt", step.id))
        } else {
            None
        };

        // Ensure per-step artifacts dir.
        let artifacts_host = ctx.artifact_path_for_step(&step.id);
        if let Err(e) = tokio::fs::create_dir_all(&artifacts_host).await {
            return StepResult::Failed {
                error: format!("mkdir artifacts dir: {e}"),
                tokens_used: 0,
            };
        }

        // Build the final bash -c command.
        // Wrap in `cd /home/sandboxuser/workflow/<run_id> && [cat input |] <run>`
        // so bundle-relative refs resolve.
        let run_dir = ctx.sandbox_run_dir_str();
        let inner = if let Some(p) = stdin_path_sandbox {
            format!("cd {run_dir} && cat {p} | {run_rendered}")
        } else {
            format!("cd {run_dir} && {run_rendered}")
        };
        let cmd = format!("bash -c {}", shell_escape_single(&inner));

        // Workflow mount + per-step artifacts mount.
        let workflow_mount = StagedMount {
            mode: StageMode::ReadOnly,
            host_path: ctx.sandbox_workspace.clone(),
            sandbox_path: run_dir.clone(),
        };
        let artifacts_mount = StagedMount {
            mode: StageMode::ReadWrite,
            host_path: artifacts_host.clone(),
            sandbox_path: format!("{}/artifacts/{}", run_dir, step.id),
        };
        let mounts = vec![workflow_mount, artifacts_mount];

        // Build SandboxContext. We use the conversation_id if set (so the
        // workspace tree lines up with chat-side semantics); for runs not
        // tied to a conversation we generate a stable per-run pseudo-conv.
        let conv_id = ctx.conversation_id.unwrap_or_else(|| {
            // Stable pseudo-conv = the run_id itself. Means the workspace
            // lives at <workspace_root>/<run_id>/workflow/<run_id>/.
            ctx.run_id
        });
        let sb_ctx = SandboxContext {
            conversation_id: conv_id,
            user_id: ctx.user_id,
            // M2: the sandbox home bind source is the conversation workspace
            // dir, i.e. two levels up from the staged run dir
            // (`<root>/<conv>/workflow/<run>` → `<root>/<conv>`). Derive it
            // deterministically from the same root + conv_id the runner used
            // to stage, instead of `parent().join("..").canonicalize()` which
            // silently fell back to the WRONG dir (the run dir itself) when
            // canonicalize failed.
            workspace: crate::modules::workflow::runner::workflow_workspace_root()
                .join(conv_id.to_string()),
            files: Arc::new(Vec::new()),
        };

        // Dispatch — wrapping in select! gives us prompt cancel via
        // future-drop (kill_on_drop(true) is already set on the sandbox
        // Command).
        let result = tokio::select! {
            r = execute_command_with_mounts(&sb_ctx, &cmd, &flavor, &mounts) => r,
            _ = cancel.await_cancel() => return StepResult::Cancelled,
        };

        let response = match result {
            Ok(v) => v,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("sandbox: {e}"),
                    tokens_used: 0,
                };
            }
        };

        let stdout = response
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let stderr = response
            .get("stderr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let exit_code = response.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);

        // Capture stderr (gated).
        let _ = log_io::write_text_log(ctx, &step.id, "stderr", &stderr, step.log).await;
        // Capture raw stdout as raw_output log (gated at full level).
        let _ = log_io::write_text_log(ctx, &step.id, "raw_output", &stdout, step.log).await;

        if exit_code != 0 {
            return StepResult::Failed {
                error: format!("sandbox exit code {exit_code}: {}", stderr.chars().take(500).collect::<String>()),
                tokens_used: 0,
            };
        }

        // Parse stdout — best-effort JSON sniff.
        let (value, parsed_as) = if let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) {
            (v, ParsedAs::Json)
        } else {
            (Value::String(stdout), ParsedAs::Text)
        };

        let meta = match file_io::write_step_output(
            ctx,
            &step.id,
            &value,
            parsed_as,
            StepKindTag::Sandbox,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("persist step output: {e}"),
                    tokens_used: 0,
                };
            }
        };
        ctx.step_outputs.insert(step.id.clone(), meta);

        StepResult::Completed {
            output: value,
            parsed_as,
            tokens_used: 0,
            ms_elapsed: started.elapsed().as_millis() as u64,
        }
    }
}

/// Single-quote bash escape: `foo` → `'foo'`; `it's` → `'it'\''s'`.
fn shell_escape_single(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

// ============================================================
// Elicit dispatcher (§4.6)
// ============================================================

pub struct ElicitDispatcher;

impl ElicitDispatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ElicitDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StepDispatcher for ElicitDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let started = Instant::now();
        let (schema, timeout_ms) = match &step.config {
            StepConfig::Elicit {
                schema,
                timeout_ms,
            } => (schema.clone(), *timeout_ms),
            _ => {
                return StepResult::Failed {
                    error: "ElicitDispatcher called on non-elicit step".into(),
                    tokens_used: 0,
                };
            }
        };
        // The elicitation prompt is the shared `StepDef.message` field.
        let message_tpl = match step.message.as_deref() {
            Some(m) => m.to_string(),
            None => {
                return StepResult::Failed {
                    error: "elicit step requires a `message` (the prompt shown to the user)".into(),
                    tokens_used: 0,
                };
            }
        };

        let message = match crate::modules::workflow::template::render(&message_tpl, ctx) {
            Ok(s) => s,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("elicit message render: {e}"),
                    tokens_used: 0,
                };
            }
        };

        let elicitation_id = Uuid::new_v4();
        let deadline = chrono::Utc::now()
            + chrono::Duration::milliseconds(timeout_ms as i64);

        // M3: set the in-memory registry slot FIRST (synchronous), THEN
        // persist to DB, THEN emit. The `/elicit` handler validates the
        // elicitation_id against the DB record and delivers via the registry
        // slot; setting the slot before the DB record guarantees that any
        // submission carrying a valid (DB-persisted) elicitation_id always
        // finds the delivery slot already present — closing the window where a
        // correctly-timed submission got a spurious 410.
        let rx = match registry::set_pending_elicitation(ctx.run_id, elicitation_id) {
            Ok(rx) => rx,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("register pending elicit: {e}"),
                    tokens_used: 0,
                };
            }
        };

        // Persist pending state to DB so a page-reload can render the form.
        if let Err(e) = persist_pending(
            ctx,
            elicitation_id,
            &step.id,
            &message,
            &schema,
            deadline,
        )
        .await
        {
            // Roll back the registry slot we just set so it can't leak.
            registry::clear_pending_elicitation(ctx.run_id);
            return StepResult::Failed {
                error: format!("persist pending elicit: {e}"),
                tokens_used: 0,
            };
        }

        emit.emit(SSEWorkflowRunEvent::ElicitationRequired(
            SSEElicitationRequiredData {
                run_id: ctx.run_id,
                step_id: step.id.clone(),
                elicitation_id,
                message,
                schema: schema.clone(),
                deadline_at: deadline,
            },
        ));

        let deadline_inst =
            std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms as u64);

        let value = tokio::select! {
            r = rx => match r {
                Ok(v) => v,
                Err(_) => {
                    let _ = clear_pending(ctx).await;
                    emit.emit(SSEWorkflowRunEvent::ElicitationResolved(SSEElicitationResolvedData {
                        run_id: ctx.run_id,
                        step_id: step.id.clone(),
                        elicitation_id,
                        resolved_by: "cancel".into(),
                    }));
                    return StepResult::Cancelled;
                }
            },
            _ = cancel.await_cancel() => {
                let _ = clear_pending(ctx).await;
                emit.emit(SSEWorkflowRunEvent::ElicitationResolved(SSEElicitationResolvedData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    elicitation_id,
                    resolved_by: "cancel".into(),
                }));
                return StepResult::Cancelled;
            }
            _ = tokio::time::sleep_until(deadline_inst.into()) => {
                let _ = clear_pending(ctx).await;
                emit.emit(SSEWorkflowRunEvent::ElicitationResolved(SSEElicitationResolvedData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    elicitation_id,
                    resolved_by: "timeout".into(),
                }));
                return StepResult::Failed {
                    error: format!("elicit timed out after {timeout_ms}ms"),
                    tokens_used: 0,
                };
            }
        };

        // Loose schema-shape check (full jsonschema-rs lands in a future
        // patch; for Phase 1 we accept any non-null JSON value — the form
        // FE side enforces the schema before posting).
        if value.is_null() {
            let _ = clear_pending(ctx).await;
            return StepResult::Failed {
                error: "elicit response was null".into(),
                tokens_used: 0,
            };
        }

        let meta = match file_io::write_step_output(
            ctx,
            &step.id,
            &value,
            ParsedAs::Json,
            StepKindTag::Elicit,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                let _ = clear_pending(ctx).await;
                return StepResult::Failed {
                    error: format!("persist elicit output: {e}"),
                    tokens_used: 0,
                };
            }
        };
        ctx.step_outputs.insert(step.id.clone(), meta);

        let _ = clear_pending(ctx).await;
        emit.emit(SSEWorkflowRunEvent::ElicitationResolved(
            SSEElicitationResolvedData {
                run_id: ctx.run_id,
                step_id: step.id.clone(),
                elicitation_id,
                resolved_by: "user".into(),
            },
        ));

        StepResult::Completed {
            output: value,
            parsed_as: ParsedAs::Json,
            tokens_used: 0,
            ms_elapsed: started.elapsed().as_millis() as u64,
        }
    }
}

async fn persist_pending(
    ctx: &RunContext,
    elicitation_id: Uuid,
    step_id: &str,
    message: &str,
    schema: &Value,
    deadline: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    let record = crate::modules::workflow::types::PendingElicitationRecord {
        elicitation_id,
        step_id: step_id.into(),
        message: message.into(),
        schema: schema.clone(),
        deadline_at: deadline,
    };
    let json = serde_json::to_value(&record)
        .map_err(|e| AppError::internal_error(format!("serialize pending elicit: {e}")))?;
    crate::modules::workflow::repository::set_pending_elicitation(
        crate::core::Repos.pool(),
        ctx.run_id,
        Some(json),
    )
    .await
}

async fn clear_pending(ctx: &RunContext) -> Result<(), AppError> {
    crate::modules::workflow::repository::set_pending_elicitation(
        crate::core::Repos.pool(),
        ctx.run_id,
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn bare_ctx() -> RunContext {
        RunContext {
            run_id: uuid::Uuid::nil(),
            user_id: uuid::Uuid::nil(),
            conversation_id: None,
            workflow_id: uuid::Uuid::nil(),
            inputs: HashMap::new(),
            step_outputs: HashMap::new(),
            step_item_progress: HashMap::new(),
            extracted_path: PathBuf::from("/tmp/_"),
            sandbox_workspace: PathBuf::from("/tmp/_/ws"),
            outputs_dir: PathBuf::from("/tmp/_/ws/outputs"),
            artifacts_dir: PathBuf::from("/tmp/_/ws/artifacts"),
            inputs_dir: PathBuf::from("/tmp/_/ws/inputs"),
            model_id: uuid::Uuid::nil(),
            model_name: "test-model".into(),
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: HashMap::new(),
            force_mocks: false,
        }
    }

    /// H4: llm_map per-item prompt rendering over OBJECT items binds the
    /// item so `{{ q.title }}` resolves to the field (not raw spliced JSON).
    /// This is the exact substitution the `LlmMapDispatcher` performs before
    /// spawning each item's LLM call.
    #[test]
    fn llm_map_item_field_renders() {
        let ctx = bare_ctx();
        let item_var = "q".to_string();
        let raw_prompt = "summarize {{ q.title }}";
        let items = vec![serde_json::json!({"title": "X"})];

        let mut rendered = Vec::new();
        for item in &items {
            let mut binding = HashMap::new();
            binding.insert(item_var.clone(), item.clone());
            rendered.push(
                crate::modules::workflow::template::render_with_bindings(
                    raw_prompt, &ctx, &binding,
                )
                .unwrap(),
            );
        }
        assert_eq!(rendered, vec!["summarize X".to_string()]);
    }

    #[test]
    fn shell_escape_single_wraps_and_escapes() {
        // Plain string → single-quoted.
        assert_eq!(shell_escape_single("hello"), "'hello'");
        // Embedded single quote → the `'\''` dance.
        assert_eq!(shell_escape_single("a'b"), "'a'\\''b'");
        // Empty string still produces a valid empty arg.
        assert_eq!(shell_escape_single(""), "''");
        // Shell metachars are inert inside single quotes.
        assert_eq!(shell_escape_single("$(rm -rf /)"), "'$(rm -rf /)'");
    }

    #[test]
    fn retry_backoff_is_monotonic_and_capped() {
        let d0 = retry_backoff(0);
        let d1 = retry_backoff(1);
        let d2 = retry_backoff(2);
        assert_eq!(d0, std::time::Duration::from_millis(250));
        assert_eq!(d1, std::time::Duration::from_millis(500));
        assert_eq!(d2, std::time::Duration::from_millis(1000));
        assert!(d1 > d0 && d2 > d1);
        // Far out → capped at 8s, never overflows the shift.
        assert_eq!(retry_backoff(100), std::time::Duration::from_millis(8_000));
    }

    #[test]
    fn classify_item_error_branches() {
        // Fail + Retry-exhausted are fatal (abort the step).
        let (fatal, val) = classify_item_error(OnError::Fail);
        assert!(fatal);
        assert!(val.is_none());
        let (fatal, val) = classify_item_error(OnError::Retry);
        assert!(fatal);
        assert!(val.is_none());
        // Skip is non-fatal and records a Null result.
        let (fatal, val) = classify_item_error(OnError::Skip);
        assert!(!fatal);
        assert_eq!(val, Some(Value::Null));
    }

    /// H4 regression: a scalar item still renders bare (no JSON quoting).
    #[test]
    fn llm_map_scalar_item_renders_bare() {
        let ctx = bare_ctx();
        let mut binding = HashMap::new();
        binding.insert("q".to_string(), serde_json::json!("hello"));
        let s = crate::modules::workflow::template::render_with_bindings(
            "say {{ q }}", &ctx, &binding,
        )
        .unwrap();
        assert_eq!(s, "say hello");
    }
}

