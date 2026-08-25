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


use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use ai_providers::{
    ChatMessage, ChatRequest, ContentBlockDelta, Provider,
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
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{
    ItemProgress, ParsedAs, RunContext, StepKindTag, StepResult,
};
use crate::modules::workflow::validate::{
    OnError, OutputFormat, PromptSource, StepConfig, StepDef, prompt_source,
};

/// Per-call LLM token cap (plan §4.5).
// FIXME(token-cap): defined per the plan but not yet enforced at any call site;
// the per-step cap below is what's wired today. Kept so the policy constant
// isn't lost before the per-call enforcement lands.
#[allow(dead_code)]
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
pub(crate) async fn resolve_prompt(
    step: &StepDef,
    ctx: &RunContext,
    prompt: &Option<String>,
    prompt_file: &Option<String>,
) -> Result<String, String> {
    let raw = load_raw_prompt(&step.id, &ctx.extracted_path, prompt, prompt_file).await?;
    crate::modules::workflow::template::render(&raw, ctx).map_err(|e| e.to_string())
}

/// Load the RAW (un-rendered) prompt from inline `prompt:` or
/// `prompt_file:`. Used by `llm_map`, whose per-item prompt contains the
/// `{{ <item_var> }}` binding that does NOT exist in `ctx` — so it must be
/// rendered per-item via `render_with_bindings` (H4), never pre-rendered
/// against `ctx` alone.
///
/// Takes `step_id` + `bundle_root` rather than the whole `RunContext`: those are
/// the only two things it needs, and the narrower signature is what lets the
/// validate↔dispatch agreement matrix (INV-1) drive this function for real
/// against a real bundle directory instead of a stand-in.
///
/// **Nothing about WHICH prompt is used is decided here.** Both halves are
/// decided by `validate.rs`: `prompt_source` picks the source, and
/// `read_prompt_file` resolves + confines + reads the file. Those are the same
/// two functions the validator's verdict is computed from, so a definition the
/// validator reports green cannot fail here, and one it reports red cannot
/// quietly succeed here (INV-1, both directions).
///
/// Both halves were separately wrong before. This function matched
/// `(Option, Option)` raw, so `Some("")` beside a `prompt_file:` read as "both"
/// and failed the run on a workflow that had just validated clean — the state
/// the builder's own `WORKFLOW_PROMPT_BOTH` remedy tells the author to create;
/// and it joined the path
/// with NO shape or confinement check, so a `prompt_file:` the validator refused
/// as `WORKFLOW_PROMPT_FILE_UNSAFE` was read here anyway. The runner does not
/// get to rely on having been validated: `spawn_run`/`resume_run` do re-validate
/// immediately before dispatch, but `POST /workflows/{id}/test` reaches dispatch
/// without `validate_for_install` at all, so a prior validation is not a
/// precondition this function may assume.
async fn load_raw_prompt(
    step_id: &str,
    bundle_root: &Path,
    prompt: &Option<String>,
    prompt_file: &Option<String>,
) -> Result<String, String> {
    match prompt_source(prompt, prompt_file) {
        PromptSource::Inline(p) => Ok(p.to_string()),
        PromptSource::File(rel) => {
            // `read_prompt_file` is sync (the validator is a sync fn), and it
            // stats + reads, so it goes on the blocking pool rather than a tokio
            // worker.
            let (root, rel_owned) = (bundle_root.to_path_buf(), rel.to_string());
            let read = tokio::task::spawn_blocking(move || {
                crate::modules::workflow::validate::read_prompt_file(&root, &rel_owned)
            })
            .await
            .map_err(|e| format!("read prompt_file '{rel}': {e}"))?;
            read.map_err(|e| e.message(rel))
        }
        // Distinguished: the enum knows which it is, and "no prompt at all" and
        // "two competing prompts" need different remedies. Only reachable when
        // validation was bypassed, which is exactly when the operator has least
        // context.
        PromptSource::Missing => Err(format!(
            "step '{step_id}': {}",
            crate::modules::workflow::validate::PROMPT_MISSING_MESSAGE
        )),
        PromptSource::Both => Err(format!(
            "step '{step_id}': {}",
            crate::modules::workflow::validate::PROMPT_BOTH_MESSAGE
        )),
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
            max_tokens: Some(ctx.model_max_tokens),
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

        let (value, parsed_as) = match parse_llm_output(&text, output_format) {
            Ok(vp) => vp,
            Err(error) => {
                return StepResult::Failed {
                    error,
                    tokens_used: tokens,
                };
            }
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
        let raw_prompt = match load_raw_prompt(&step.id, &ctx.extracted_path, &prompt, &prompt_file)
            .await
        {
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
            item_progress(0, total, 0, 0, 0),
        );
        emit.emit(SSEWorkflowRunEvent::StepItemProgress(SSEStepItemProgressData {
            run_id: ctx.run_id,
            step_id: step.id.clone(),
            progress: ctx.step_item_progress[&step.id].clone(),
        }));

        let max_toks = ctx.model_max_tokens;
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
                let mut last_err: String;
                let mut total_tokens: u64 = 0;
                loop {
                    attempts += 1;
                    let req = ChatRequest {
                        model: model.clone(),
                        messages: vec![ChatMessage::user(prompt.clone())],
                        max_tokens: Some(max_toks),
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
                let progress = item_progress(completed, total, failed, skipped, total_tokens);
                tracing::info!(
                    step = %step.id,
                    cancelled = cancelled_so_far(&progress),
                    "llm_map step cancelled mid-drain"
                );
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
            let progress = item_progress(completed, total, failed, skipped, total_tokens);
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

/// Map a raw LLM response into a typed step value per the declared output
/// format (E6). `OutputFormat::Json` with unparseable text is a step failure;
/// `Text` always succeeds. Factored from the inline `match` so the parse-fail
/// branch is unit-testable without a real LLM call.
pub(crate) fn parse_llm_output(text: &str, output_format: OutputFormat) -> Result<(Value, ParsedAs), String> {
    match output_format {
        OutputFormat::Text => Ok((Value::String(text.to_string()), ParsedAs::Text)),
        OutputFormat::Json => serde_json::from_str::<Value>(text)
            .map(|v| (v, ParsedAs::Json))
            .map_err(|e| format!("expected JSON output, parse failed: {e}")),
    }
}

/// Build the per-item progress snapshot for an llm_map fan-out. Used both for
/// the running per-item updates and for the partial-progress snapshot taken
/// when the step is cancelled mid-drain (E4) — the cancelled count is
/// derivable as `total - completed - failed - skipped`. Factored so the
/// snapshot bookkeeping is unit-testable without racing a real fan-out.
fn item_progress(
    completed: u32,
    total: u32,
    failed: u32,
    skipped: u32,
    tokens_so_far: u64,
) -> ItemProgress {
    ItemProgress {
        completed,
        total,
        failed,
        skipped,
        tokens_so_far,
    }
}

/// Items neither completed, failed, nor skipped when an llm_map step is
/// cancelled mid-drain — the "cancelled" count the snapshot leaves derivable
/// (`total - completed - failed - skipped`, saturating). Pure, so the
/// derivation is unit-testable.
fn cancelled_so_far(p: &ItemProgress) -> u32 {
    p.total
        .saturating_sub(p.completed)
        .saturating_sub(p.failed)
        .saturating_sub(p.skipped)
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
        // Caller-injected `/lit` full-text view bind, computed ziee-side (where
        // `lit_search` is reachable) and threaded through the build-DB-free
        // sandbox engine via `SandboxContext::extra_ro_binds` — byte-identical to
        // the former inline `build_bwrap_argv` block, which computed the same
        // host path from `conversation_id`.
        let mut extra_ro_binds: Vec<(String, String)> = Vec::new();
        {
            let lit_view =
                crate::modules::lit_search::fulltext::cache::conversation_view_dir(conv_id);
            if lit_view.is_dir() {
                extra_ro_binds.push((
                    lit_view.display().to_string(),
                    crate::modules::lit_search::fulltext::cache::SANDBOX_MOUNT_PATH.to_string(),
                ));
            }
        }
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
            extra_ro_binds,
        };

        // Live progress (P2): a per-step sink feeds the consumer, which parses
        // `$ZIEE_PROGRESS` lines, coalesces per track, and emits `StepProgress`.
        // The sink is moved into the exec; when the exec future completes (or is
        // dropped on cancel) the sender drops, the consumer's `rx` closes, it
        // does a final flush, and ends.
        let progress_pool = crate::core::repository::Repos.pool().clone();
        let (progress_tx, progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let progress_consumer =
            tokio::spawn(crate::modules::workflow::sandbox_progress::run_progress_consumer(
                progress_rx,
                emit.clone(),
                progress_pool.clone(),
                ctx.run_id,
                step.id.clone(),
            ));

        // Dispatch — wrapping in select! gives us prompt cancel via
        // future-drop (kill_on_drop(true) is already set on the sandbox
        // Command).
        let result = tokio::select! {
            r = execute_command_with_mounts(
                &sb_ctx, &cmd, &flavor, &mounts, Some(progress_tx),
            ) => r,
            _ = cancel.await_cancel() => {
                // P2.8: tear the consumer down so no progress frames fire after
                // cancel (the exec future drops here, killing the sandbox), and
                // clear the live-progress slot ourselves (the aborted consumer
                // can't run its own end-clear).
                progress_consumer.abort();
                let _ = crate::modules::workflow::repository::clear_step_progress(
                    &progress_pool,
                    ctx.run_id,
                )
                .await;
                return StepResult::Cancelled;
            }
        };
        // Exec finished → the sink dropped inside it → drain + flush + clear the
        // consumer before we move on.
        let _ = progress_consumer.await;

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
// Tool dispatcher (A6 — call an MCP tool on an accessible server)
// ============================================================

pub struct ToolDispatcher;

impl ToolDispatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToolDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a tool `arguments` JSON value against `ctx`. A string that is exactly
/// a single `{{ ref }}` resolves to its NATIVE JSON type (number/array/object
/// pass through typed); a string with surrounding text does interpolation;
/// literals pass through. Arrays/objects recurse.
pub(crate) fn render_tool_arguments(args: &Value, ctx: &RunContext) -> Result<Value, String> {
    match args {
        Value::String(s) => {
            let trimmed = s.trim();
            let whole_value_ref = trimmed.starts_with("{{")
                && trimmed.ends_with("}}")
                && trimmed.matches("{{").count() == 1;
            if whole_value_ref {
                let rendered = crate::modules::workflow::template::render(s, ctx)
                    .map_err(|e| e.to_string())?;
                // Native type when the rendered text is valid JSON; else string.
                Ok(serde_json::from_str::<Value>(&rendered).unwrap_or(Value::String(rendered)))
            } else if s.contains("{{") {
                let rendered = crate::modules::workflow::template::render(s, ctx)
                    .map_err(|e| e.to_string())?;
                Ok(Value::String(rendered))
            } else {
                Ok(args.clone())
            }
        }
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for e in a {
                out.push(render_tool_arguments(e, ctx)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, v) in m {
                out.insert(k.clone(), render_tool_arguments(v, ctx)?);
            }
            Ok(Value::Object(out))
        }
        other => Ok(other.clone()),
    }
}

/// Concatenate the `text` content blocks of a tool result.
fn tool_result_text(result: &crate::modules::mcp::client::traits::ToolResult) -> String {
    let mut parts = Vec::new();
    for c in &result.content {
        if c.content.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(t) = c.content.get("text").and_then(|v| v.as_str()) {
                parts.push(t.to_string());
            }
        }
    }
    parts.join("\n")
}


// The shared MCP tool-call chokepoint (call_mcp_tool + resolve_tool_server +
// built-in name map + McpCallScope/McpToolCallError/CancelSignal/ChatCallCtx)
// now lives in `mcp::agent_tool_call` (shared infra) so BOTH this dispatcher and
// the chat agent host import it from `mcp/`, not from each other (§9 DAG).
// Re-exported for this module's internal callers.
pub(crate) use crate::modules::mcp::agent_tool_call::{
    call_mcp_tool, CancelSignal, McpCallScope, McpToolCallError,
};

// `RunHandle` (workflow-owned) implements the shared `CancelSignal` trait — the
// one workflow-local binding kept here (orphan rule: workflow owns RunHandle).
#[async_trait]
impl CancelSignal for registry::RunHandle {
    async fn cancelled(&self) {
        self.await_cancel().await;
    }
}


#[async_trait]
impl StepDispatcher for ToolDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let _ = emit;
        let started = Instant::now();

        let (server_name, tool_name, arguments) = match &step.config {
            StepConfig::Tool {
                server,
                tool,
                arguments,
            } => (server.clone(), tool.clone(), arguments.clone()),
            _ => {
                return StepResult::Failed {
                    error: "ToolDispatcher called on non-tool step".into(),
                    tokens_used: 0,
                };
            }
        };

        let args = match render_tool_arguments(&arguments, ctx) {
            Ok(v) => v,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("arguments: render failed: {e}"),
                    tokens_used: 0,
                };
            }
        };

        // ITEM-21 / DEC-17: the resolve → disabled-gate → session → call path is
        // the shared `call_mcp_tool` impl. The tool step passes
        // `enforce_conversation_disabled = true` — same gate it applied inline
        // before the extraction (behaviour-preserving; the E8 conversation +
        // scheduled/default disabled-server checks live inside the helper now).
        let scope = McpCallScope {
            user_id: ctx.user_id,
            conversation_id: ctx.conversation_id,
            run_id: ctx.run_id,
        };
        let (server_id, tool_result) =
            match call_mcp_tool(&scope, &server_name, &tool_name, args, true, cancel.as_ref(), None /*chat_ctx*/, None, None,
                crate::modules::mcp::tool_calls::models::McpToolCallSource::Workflow,
                None /* timing_out — no live tool-lifecycle SSE on the workflow surface */)
                .await
            {
                Ok(v) => v,
                Err(McpToolCallError::Cancelled) => return StepResult::Cancelled,
                Err(McpToolCallError::Failed(error)) => {
                    return StepResult::Failed { error, tokens_used: 0 };
                }
            };

        // Log the raw result (gated). On a serialize failure, record the
        // error context rather than a silent empty string.
        let raw = serde_json::to_string(&tool_result.content)
            .unwrap_or_else(|e| format!("<failed to serialize tool result content: {e}>"));
        let _ = log_io::write_text_log(ctx, &step.id, "raw_output", &raw, step.log).await;

        if tool_result.is_error {
            let msg = tool_result_text(&tool_result);
            return StepResult::Failed {
                error: format!(
                    "tool '{tool_name}' returned an error: {}",
                    msg.chars().take(500).collect::<String>()
                ),
                tokens_used: 0,
            };
        }

        // C3/C4/E9: persist any `resource_link` files the tool returned into
        // durable file-store artifacts (created_by="workflow", linked to the run
        // via workflow_run_id for the A5 cascade). Mirrors the chat path
        // (mcp/chat_extension/mcp.rs): `is_saved:true` links are referenced,
        // `ziee://<host_path>` links from trusted built-ins are read off disk
        // behind path-confinement, and http:// loopback links are fetched with a
        // short-lived JWT (E9 — the dispatcher passes the manager's secret).
        let mut tool_files: Vec<Value> = Vec::new();
        {
            let mut links: Vec<crate::modules::mcp::chat_extension::content::ResourceLink> =
                tool_result
                    .content
                    .iter()
                    .filter(|b| {
                        b.content.get("type").and_then(|t| t.as_str()) == Some("resource_link")
                    })
                    .filter_map(|b| {
                        crate::modules::mcp::resource_link::parse_resource_link_block(&b.content)
                    })
                    .collect();
            if !links.is_empty() {
                // The session manager is re-fetched here (the call path itself
                // now lives in `call_mcp_tool`); it supplies the E9 JWT for
                // loopback resource-link fetches. Absent ⇒ skip persistence.
                let Some(manager) = crate::modules::mcp::client::manager::global() else {
                    return StepResult::Failed {
                        error: "MCP session manager not initialized".into(),
                        tokens_used: 0,
                    };
                };
                // `ziee://` reads are confined to (a) this run's OWN workflow
                // staging dir, and (b) the code_sandbox workspace for this run's
                // key (the common producer, get_resource_link). Including (a)
                // means artifacts still persist in a deployment where
                // code_sandbox isn't initialized (else allowed_roots would be
                // empty and every ziee:// link would be silently dropped).
                let sandbox_key = ctx.conversation_id.unwrap_or(ctx.run_id);
                let mut allowed_roots: Vec<std::path::PathBuf> =
                    vec![ctx.sandbox_workspace.clone()];
                if let Some(s) = crate::modules::code_sandbox::config::get_state() {
                    allowed_roots.push(s.workspace_root.join(sandbox_key.to_string()));
                }
                let (is_built_in, headers) =
                    match crate::core::repository::Repos.mcp.get_any_server(server_id).await {
                        Ok(Some(s)) => (s.is_built_in, s.headers),
                        _ => (false, serde_json::json!({})),
                    };
                // Same-host trust set (see `resource_link::result_link_trusted_hosts`): hosts of the
                // user's enabled accessible NON-built-in MCP servers, so an external server's artifact
                // URL on its own private host (e.g. `host.docker.internal`) can be ingested — no
                // ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE opt-in needed. A built-in emitter (e.g.
                // code_sandbox `ziee://` artifacts — the common workflow producer) short-circuits to
                // empty and skips the DB query.
                let trusted_hosts = crate::modules::mcp::resource_link::result_link_trusted_hosts(
                    is_built_in,
                    ctx.user_id,
                )
                .await;
                let outcome = crate::modules::mcp::resource_link::persist_links(
                    &mut links,
                    ctx.user_id,
                    ctx.conversation_id,
                    None, // message_id
                    "workflow",
                    Some(ctx.run_id), // C4: link each ingested file to the run
                    server_id,
                    is_built_in,
                    &headers,
                    &trusted_hosts,
                    &allowed_roots,
                    Some(manager.jwt_secret()), // E9
                    Some(manager.jwt_issuer()),
                    Some(manager.jwt_audience()),
                )
                .await
                .unwrap_or_default();
                for art in &outcome.saved {
                    tool_files.push(serde_json::json!({
                        "file_id": art.file_id,
                        "filename": art.filename,
                        "mime_type": art.mime_type,
                        "uri": format!("/api/files/{}", art.file_id),
                    }));
                }
                for (name, uri) in &outcome.referenced {
                    tool_files.push(serde_json::json!({ "filename": name, "uri": uri }));
                }
            }
        }

        // Capture: structuredContent → JSON; else concatenated text blocks
        // (best-effort JSON sniff so a JSON-returning tool stays typed).
        let (value, parsed_as) = if let Some(sc) = tool_result.structured_content.clone() {
            (sc, ParsedAs::Json)
        } else {
            let text = tool_result_text(&tool_result);
            if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                (v, ParsedAs::Json)
            } else {
                (Value::String(text), ParsedAs::Text)
            }
        };

        // Surface persisted files to downstream steps as `output.files[]`
        // ({{ step.output.files[0].uri }}). Merge into an object result; wrap a
        // scalar/text result so the files stay addressable without clobbering it.
        let (value, parsed_as) = if tool_files.is_empty() {
            (value, parsed_as)
        } else if let Value::Object(mut map) = value {
            let key = if map.contains_key("files") {
                "_ziee_files"
            } else {
                "files"
            };
            map.insert(key.to_string(), Value::Array(tool_files));
            (Value::Object(map), ParsedAs::Json)
        } else {
            (
                serde_json::json!({ "output": value, "files": tool_files }),
                ParsedAs::Json,
            )
        };

        let meta =
            match file_io::write_step_output(ctx, &step.id, &value, parsed_as, StepKindTag::Tool)
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
        let (schema, data_tpl, timeout_ms) = match &step.config {
            StepConfig::Elicit {
                schema,
                data,
                timeout_ms,
            } => (schema.clone(), data.clone(), *timeout_ms),
            _ => {
                return StepResult::Failed {
                    error: "ElicitDispatcher called on non-elicit step".into(),
                    tokens_used: 0,
                };
            }
        };

        // === DURABLE RESUME (Change B) ===
        // If a response was submitted while no runner was resident (a cold
        // `timeout_ms: 0` gate, e.g. post-restart), `submit_elicit` persisted it
        // on the run row + spawned this resume. Consume it here instead of
        // re-presenting the gate. Stored shape: `{ step_id, elicitation_id,
        // response }`; consumed only for THIS step, exactly once.
        {
            let pool = crate::core::Repos.pool();
            if let Ok(Some(resp_json)) = repository::get_elicit_response(pool, ctx.run_id).await
                && resp_json.get("step_id").and_then(|v| v.as_str()) == Some(step.id.as_str())
            {
                let eid = resp_json
                    .get("elicitation_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let value = resp_json.get("response").cloned().unwrap_or(Value::Null);
                // Flip `waiting` → `running` (continuing the burst) and clear
                // the durable response so it is consumed exactly once.
                let _ = repository::mark_status(
                    pool,
                    ctx.run_id,
                    crate::modules::workflow::models::WorkflowRunStatus::Running,
                    None,
                )
                .await;
                let _ = repository::set_elicit_response(pool, ctx.run_id, None).await;
                return finish_elicit(ctx, &step.id, &emit, eid, value, started).await;
            }
        }

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

        // D2: render the optional `data:` seed with the SAME type-preserving
        // renderer as `tool` arguments (a whole-value `{{ ref }}` → native JSON),
        // so a prior step's output (e.g. an AI screening table) pre-fills the form.
        let data = match data_tpl {
            Some(d) => match render_tool_arguments(&d, ctx) {
                Ok(v) => Some(v),
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("elicit data render: {e}"),
                        tokens_used: 0,
                    };
                }
            },
            None => None,
        };

        let elicitation_id = Uuid::new_v4();
        // `timeout_ms == 0` is the "no timeout — wait indefinitely" sentinel: a
        // DURABLE gate. Rather than hold a resident task (which the fixed run
        // wall-clock would kill at 30 min, and which wouldn't survive a
        // restart), we persist the pending record, flip the run to `waiting`,
        // and SUSPEND below — it resumes when the user submits (`submit_elicit`
        // → `resume_run`). A far-future `deadline_at` keeps the submit handler's
        // deadline check passing however long the human takes.
        let deadline = if timeout_ms == 0 {
            chrono::Utc::now() + chrono::Duration::days(365 * 100)
        } else {
            chrono::Utc::now() + chrono::Duration::milliseconds(timeout_ms as i64)
        };

        if timeout_ms == 0 {
            // DURABLE GATE → SUSPEND. No registry slot (no resident task to
            // deliver to). Persist the pending record (a reload / resume
            // re-renders the form), mark `waiting`, emit, and return Suspended.
            //
            // v1 scope note: this targets STANDALONE runs (REST `/run`, fire-
            // and-forget). A run invoked FROM CHAT via `workflow_mcp` blocks in
            // `await_terminal`, whose no-progress guard treats a stale
            // `updated_at` as a crash — a suspended run has no heartbeat, so a
            // chat-invoked `timeout_ms: 0` gate is cancelled after ~5 min. That
            // is acceptable for v1 (the blocking-tool model can't wait days);
            // durable, human-paced reviews run standalone.
            if let Err(e) = persist_pending(
                ctx,
                elicitation_id,
                &step.id,
                &message,
                &schema,
                data.as_ref(),
                deadline,
            )
            .await
            {
                return StepResult::Failed {
                    error: format!("persist pending elicit: {e}"),
                    tokens_used: 0,
                };
            }
            let _ = repository::mark_status(
                crate::core::Repos.pool(),
                ctx.run_id,
                crate::modules::workflow::models::WorkflowRunStatus::Waiting,
                None,
            )
            .await;
            emit.emit(SSEWorkflowRunEvent::ElicitationRequired(
                SSEElicitationRequiredData {
                    run_id: ctx.run_id,
                    step_id: step.id.clone(),
                    elicitation_id,
                    message,
                    schema: schema.clone(),
                    data: data.clone(),
                    deadline_at: deadline,
                },
            ));
            return StepResult::Suspended;
        }

        // BOUNDED GATE (`timeout_ms > 0`) → resident park (unchanged behavior).
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
            data.as_ref(),
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
                data: data.clone(),
                deadline_at: deadline,
            },
        ));

        // `timeout_ms > 0` here (the `== 0` durable case suspended above).
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

        finish_elicit(ctx, &step.id, &emit, elicitation_id, value, started).await
    }
}

/// Shared elicit-resolution tail: validate non-null, write the response as the
/// step output, clear the pending record, emit `ElicitationResolved`, and return
/// `Completed`. Called by both the resident-park resolve path and the durable
/// resume-consume path (Change B), so they stay in lockstep.
async fn finish_elicit(
    ctx: &mut RunContext,
    step_id: &str,
    emit: &Arc<dyn ProgressEmitter>,
    elicitation_id: Uuid,
    value: Value,
    started: Instant,
) -> StepResult {
    // E5: full jsonschema validation runs at the SUBMIT handler
    // (handlers/elicit.rs `validate_response_shape` → 422 on mismatch), so a
    // delivered response already conforms. This null-guard is the post-handler
    // fallback (a null can only arrive via a non-handler path).
    if value.is_null() {
        let _ = clear_pending(ctx).await;
        return StepResult::Failed {
            error: "elicit response was null".into(),
            tokens_used: 0,
        };
    }

    let meta = match file_io::write_step_output(
        ctx,
        step_id,
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
    ctx.step_outputs.insert(step_id.to_string(), meta);

    let _ = clear_pending(ctx).await;
    emit.emit(SSEWorkflowRunEvent::ElicitationResolved(
        SSEElicitationResolvedData {
            run_id: ctx.run_id,
            step_id: step_id.to_string(),
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

#[allow(clippy::too_many_arguments)]
async fn persist_pending(
    ctx: &RunContext,
    elicitation_id: Uuid,
    step_id: &str,
    message: &str,
    schema: &Value,
    data: Option<&Value>,
    deadline: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    let record = crate::modules::workflow::types::PendingElicitationRecord {
        run_id: ctx.run_id,
        elicitation_id,
        step_id: step_id.into(),
        message: message.into(),
        schema: schema.clone(),
        data: data.cloned(),
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
    use crate::modules::mcp::agent_tool_call::builtin_server_id_by_name;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Workflow ↔ memory (and other built-in) wiring (gap 91d9): a workflow
    /// `tool` step with `server: memory` must resolve to the memory MCP
    /// built-in id, so a workflow can read/write user_memories via remember/
    /// recall. Also pins the rest of the built-in name map + rejects unknowns.
    #[test]
    fn builtin_name_map_includes_memory_and_rejects_unknown() {
        assert_eq!(
            builtin_server_id_by_name("memory"),
            Some(crate::modules::memory_mcp::memory_mcp_server_id()),
            "workflow must resolve the 'memory' built-in to the memory MCP server id"
        );
        // Other built-ins the workflow runner exposes.
        for (name, id) in [
            ("web_search", crate::modules::web_search::web_search_server_id()),
            ("lit_search", crate::modules::lit_search::lit_search_server_id()),
            ("citations", crate::modules::citations::citations_server_id()),
            ("files", crate::modules::files_mcp::files_mcp_server_id()),
            ("code_sandbox", crate::modules::code_sandbox::code_sandbox_server_id()),
            ("bio", crate::modules::bio_mcp::bio_mcp_server_id()),
        ] {
            assert_eq!(builtin_server_id_by_name(name), Some(id), "built-in {name}");
        }
        // A user/system server name is NOT a built-in (resolved via the DB path).
        assert_eq!(builtin_server_id_by_name("some-user-server"), None);
        assert_eq!(builtin_server_id_by_name(""), None);
    }

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
            model_max_tokens: 8192,
            sandbox_flavor: None,
            total_tokens: 0,
            total_output_bytes: 0,
            is_dev: false,
            mocks: HashMap::new(),
            force_mocks: false,
            persist_artifacts: false,
            force_log_capture: false,
            total_log_bytes: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn ctx_with_inputs(pairs: &[(&str, Value)]) -> RunContext {
        let mut ctx = bare_ctx();
        for (k, v) in pairs {
            ctx.inputs.insert(k.to_string(), v.clone());
        }
        ctx
    }

    #[test]
    fn render_tool_arguments_preserves_native_types() {
        let ctx = ctx_with_inputs(&[
            ("topic", Value::String("quantum batteries".into())),
            ("n", serde_json::json!(10)),
            ("items", serde_json::json!(["a", "b"])),
        ]);
        let args = serde_json::json!({
            "query": "{{ inputs.topic }}",        // whole-value → string
            "max_results": "{{ inputs.n }}",      // whole-value → NUMBER (not "10")
            "list": "{{ inputs.items }}",         // whole-value → ARRAY
            "label": "about {{ inputs.topic }}",  // embedded → interpolated string
            "literal": 20                          // literal passthrough
        });
        let out = render_tool_arguments(&args, &ctx).unwrap();
        assert_eq!(out["query"], serde_json::json!("quantum batteries"));
        assert_eq!(out["max_results"], serde_json::json!(10));
        assert_eq!(out["list"], serde_json::json!(["a", "b"]));
        assert_eq!(out["label"], serde_json::json!("about quantum batteries"));
        assert_eq!(out["literal"], serde_json::json!(20));
    }

    #[test]
    fn parse_llm_output_text_json_and_failure() {
        // E6: Text always succeeds; valid JSON parses; invalid JSON under
        // OutputFormat::Json is a step failure (the factored decision).
        let (v, p) = parse_llm_output("hello", OutputFormat::Text).unwrap();
        assert!(matches!(p, ParsedAs::Text));
        assert_eq!(v, Value::String("hello".into()));

        let (v, p) = parse_llm_output(r#"{"a":1}"#, OutputFormat::Json).unwrap();
        assert!(matches!(p, ParsedAs::Json));
        assert_eq!(v["a"], serde_json::json!(1));

        let err = parse_llm_output("not json", OutputFormat::Json).unwrap_err();
        assert!(err.contains("parse failed"), "json-parse-fail message: {err}");
    }

    #[test]
    fn item_progress_snapshot_and_cancelled_count() {
        // E4: the cancel-mid-drain snapshot records completed/failed/skipped +
        // tokens, and `cancelled_so_far` (the production derivation logged at
        // the cancel site) reports the items still in-flight/unstarted.
        let p = item_progress(4, 10, 1, 2, 1234);
        assert_eq!((p.completed, p.total, p.failed, p.skipped), (4, 10, 1, 2));
        assert_eq!(p.tokens_so_far, 1234);
        assert_eq!(cancelled_so_far(&p), 3, "3 items in-flight/unstarted at cancel");
        // Saturating: an over-counted snapshot never underflows to a huge u32.
        assert_eq!(cancelled_so_far(&item_progress(10, 5, 0, 0, 0)), 0);
    }

    #[test]
    fn tool_result_text_concatenates_text_blocks() {
        use crate::modules::mcp::client::traits::{ToolContent, ToolResult};
        let r = ToolResult {
            content: vec![
                ToolContent {
                    content: serde_json::json!({"type": "text", "text": "hello"}),
                },
                ToolContent {
                    content: serde_json::json!({"type": "image", "data": "x"}),
                },
                ToolContent {
                    content: serde_json::json!({"type": "text", "text": "world"}),
                },
            ],
            is_error: false,
            structured_content: None,
        };
        assert_eq!(tool_result_text(&r), "hello\nworld");
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

    // ───────────────────────── INV-1: validate ⇔ dispatch ─────────────────────
    //
    // The validator and the runner must answer "where does this step's prompt
    // come from, and can it be read" the SAME way. They did not: a step carrying
    // `prompt: ""` beside a `prompt_file:` validated GREEN (the validator
    // normalised an empty prompt to "absent") and then failed the RUN with `has
    // invalid prompt config` (the runner matched `(Option, Option)` raw); and a
    // `prompt_file:` the validator refused as UNSAFE was joined and read by the
    // runner with no path check at all.

    /// A bundle holding every kind of `prompt_file:` target a definition can
    /// name — including the ones that used to split the two sides.
    ///
    /// Returns the temp dir (kept alive by the caller) and the BUNDLE ROOT,
    /// which is a SUBDIRECTORY of it — so `prompts/escape.md` can point at a
    /// file that is genuinely outside the bundle. A fixture whose "outside" file
    /// lives under the root does not exercise confinement at all: it resolves
    /// cleanly and both sides agree for the wrong reason.
    fn prompt_bundle() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("bundle");
        std::fs::create_dir_all(root.join("prompts")).unwrap();
        std::fs::write(root.join("prompts/real.md"), "FILE PROMPT BODY").unwrap();
        std::fs::create_dir_all(root.join("prompts/adir")).unwrap();
        // Zero-byte: readable, but there is no prompt in it.
        std::fs::write(root.join("prompts/empty.md"), "").unwrap();
        // Not valid UTF-8: `is_file()` says yes, `read_to_string` says no.
        std::fs::write(root.join("prompts/binary.bin"), [0xff_u8, 0xfe, 0x00, 0x01]).unwrap();
        // A symlink pointing OUT of the bundle — shape-clean, confinement-dirty.
        // Two flavours, because they defeat DIFFERENT checks: `escape.md` is a
        // FINAL-component symlink (caught by `O_NOFOLLOW` alone), while
        // `escapedir/` is an INTERMEDIATE one — a `canonicalize` + `starts_with`
        // + `O_NOFOLLOW` sequence can be raced into following it, so only a
        // single kernel-confined resolution refuses it.
        #[cfg(unix)]
        {
            let outside = tmp.path().join("outside.md");
            std::fs::write(&outside, "OUTSIDE THE BUNDLE").unwrap();
            std::os::unix::fs::symlink(&outside, root.join("prompts/escape.md")).unwrap();
            std::os::unix::fs::symlink(tmp.path(), root.join("escapedir")).unwrap();
        }
        (tmp, root)
    }

    /// Every finding code that answers "is this step's prompt configuration
    /// usable". These are the verdicts INV-1 binds to the run outcome; any other
    /// finding (a bad ref, a missing output) is a different question.
    const PROMPT_CODES: &[&str] = &[
        "WORKFLOW_PROMPT_BOTH",
        "WORKFLOW_PROMPT_MISSING",
        "WORKFLOW_PROMPT_FILE_MISSING",
        "WORKFLOW_PROMPT_FILE_UNSAFE",
        "WORKFLOW_PROMPT_FILE_ESCAPE",
    ];

    /// Guard on the list above: the validator must not gain a prompt-related
    /// code that this matrix silently stops covering. Without it, a new
    /// `WORKFLOW_PROMPT_*` verdict would be invisible to TEST-1 — the matrix
    /// would keep passing while the class it guards had grown.
    ///
    /// Checked against `validate::VALIDATION_CODES`, the module's own canonical
    /// registry (which its `humanisation_contract` guard already proves is
    /// complete and matches the real emit sites), NOT against a text scan of the
    /// source — a scan would read this file's and validate.rs's own test
    /// modules and end up comparing the tests to themselves.
    #[test]
    fn prompt_codes_list_covers_every_prompt_verdict_the_validator_emits() {
        let mut registered: Vec<&str> = crate::modules::workflow::validate::VALIDATION_CODES
            .iter()
            .copied()
            .filter(|c| c.starts_with("WORKFLOW_PROMPT"))
            .collect();
        registered.sort_unstable();
        let mut covered: Vec<&str> = PROMPT_CODES.to_vec();
        covered.sort_unstable();
        assert_eq!(
            registered, covered,
            "the validator's registered WORKFLOW_PROMPT_* codes differ from the set \
             the INV-1 matrix treats as prompt verdicts — add the new code to \
             PROMPT_CODES *and* give the matrix an input that produces it, or the \
             agreement assertion will silently stop covering that verdict"
        );
    }

    /// Render one YAML step of `kind` carrying the given prompt fields.
    /// `None` = the key is ABSENT (not empty) — the distinction the whole
    /// defect turns on.
    fn step_yaml(kind: &str, prompt: Option<&str>, prompt_file: Option<&str>) -> String {
        let mut y = format!("steps:\n  - id: s\n    kind: {kind}\n");
        if kind == "llm_map" {
            y.push_str("    for_each: \"{{ inputs.xs }}\"\n    item_var: it\n");
        }
        if let Some(p) = prompt {
            y.push_str(&format!("    prompt: \"{p}\"\n"));
        }
        if let Some(f) = prompt_file {
            y.push_str(&format!("    prompt_file: \"{f}\"\n"));
        }
        y
    }

    /// **TEST-1 — INV-1 acceptance.**
    ///
    /// Over the whole authorable state space, the REAL validator
    /// (`validate_collecting`, against a REAL materialized bundle) and the REAL
    /// runner (`load_raw_prompt`, against the same bundle) must AGREE:
    ///
    /// * no prompt finding  ⇒ the run resolves a prompt, and
    /// * a prompt finding   ⇒ the run refuses to resolve one.
    ///
    /// This asserts the design's promise, not either side's behaviour: it goes
    /// red if EITHER the validator or the runner changes its rule unilaterally.
    /// The `files` list deliberately includes the path shapes the runner used to
    /// read without any check (`..`, absolute, an escaping symlink) and the file
    /// contents an existence-only check said yes to (a directory, non-UTF-8,
    /// zero-byte) — those are the rows where the implication used to fail.
    #[tokio::test]
    async fn validate_and_dispatch_agree_on_every_prompt_state() {
        let (_tmp, root) = prompt_bundle();
        let prompts: [Option<&str>; 4] = [None, Some(""), Some("   "), Some("hi")];
        let files: [Option<&str>; 10] = [
            None,
            Some(""),
            Some("prompts/real.md"),
            Some("prompts/adir"),
            Some("prompts/nope.md"),
            Some("prompts/empty.md"),
            Some("prompts/binary.bin"),
            Some("prompts/../prompts/real.md"),
            Some("prompts/escape.md"),
            Some("escapedir/outside.md"),
        ];

        let mut checked = 0usize;
        let mut ran_ok = 0usize;
        for kind in ["llm", "llm_map", "agent"] {
            for prompt in prompts {
                for file in files {
                    let yaml = step_yaml(kind, prompt, file);
                    let wf = crate::modules::workflow::validate::parse_workflow_yaml(&yaml)
                        .unwrap_or_else(|e| panic!("parse {kind} {prompt:?} {file:?}: {e:?}"));

                    let findings =
                        crate::modules::workflow::validate::validate_collecting(
                            &wf,
                            root.as_path(),
                            false,
                        );
                    let prompt_findings: Vec<&str> = findings
                        .iter()
                        .filter(|e| PROMPT_CODES.contains(&e.code))
                        .map(|e| e.code)
                        .collect();

                    let (p, f) = wf.steps[0]
                        .config
                        .prompt_fields()
                        .map(|(p, f)| (p.clone(), f.clone()))
                        .expect("llm/llm_map/agent carry the prompt pair");
                    // The step-level accessor must agree with the free function
                    // the runner is about to be driven with — otherwise the
                    // matrix would be proving something about a call shape no
                    // production code uses.
                    assert_eq!(
                        wf.steps[0].config.prompt_source(),
                        Some(crate::modules::workflow::validate::prompt_source(&p, &f)),
                        "StepConfig::prompt_source must equal prompt_source(prompt, prompt_file)"
                    );
                    let run = load_raw_prompt("s", root.as_path(), &p, &f).await;

                    assert_eq!(
                        prompt_findings.is_empty(),
                        run.is_ok(),
                        "kind={kind} prompt={prompt:?} prompt_file={file:?}: validator said \
                         {} but the run said {} — a definition that validates clean must run, \
                         and one the validator rejects must not quietly run (INV-1). \
                         findings={prompt_findings:?} run={run:?}",
                        if prompt_findings.is_empty() { "OK" } else { "REJECTED" },
                        if run.is_ok() { "OK" } else { "REJECTED" },
                    );
                    checked += 1;
                    if run.is_ok() {
                        ran_ok += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 120, "the matrix must actually have been walked");
        // Anti-vacuity: the implication is trivially true if NOTHING validates
        // clean. Per kind the 6 legitimately-runnable rows are
        // {absent, ""} x real.md  and  {"   ", "hi"} x {absent, ""}.
        assert_eq!(
            ran_ok, 18,
            "the matrix must contain rows that legitimately RUN, or the \
             implication holds vacuously"
        );
    }

    /// **TEST-6** — the individual cells FIX_ROUND-8 named, with their exact
    /// outcomes, so a silently-different failure cannot pass TEST-1's
    /// implication by failing on both sides for the wrong reason.
    #[tokio::test]
    async fn load_raw_prompt_reads_the_file_when_the_prompt_box_was_cleared() {
        let (_tmp, root) = prompt_bundle();

        // The exact state the builder's own WORKFLOW_PROMPT_BOTH remedy produced.
        let body = load_raw_prompt(
            "llm_1",
            root.as_path(),
            &Some(String::new()),
            &Some("prompts/real.md".into()),
        )
        .await
        .expect("an empty prompt beside a prompt_file must resolve to the FILE");
        assert_eq!(body, "FILE PROMPT BODY");

        // An empty prompt with NO file is `WORKFLOW_PROMPT_MISSING` at validate,
        // so the run must refuse rather than send an empty prompt to the model —
        // and must SAY which of the two invalid shapes it is.
        let err = load_raw_prompt("llm_1", root.as_path(), &Some(String::new()), &None)
            .await
            .expect_err("an empty prompt with no prompt_file must not resolve");
        assert_eq!(err, "step 'llm_1': step has neither prompt: nor prompt_file:");

        // A genuine both-state is still rejected, with its own distinct message.
        let err = load_raw_prompt(
            "llm_1",
            root.as_path(),
            &Some("inline".into()),
            &Some("prompts/real.md".into()),
        )
        .await
        .expect_err("prompt: and prompt_file: remain mutually exclusive");
        assert_eq!(
            err,
            "step 'llm_1': step has both prompt: and prompt_file: (mutually exclusive)"
        );

        // An inline prompt still wins when there is no file.
        assert_eq!(
            load_raw_prompt("llm_1", root.as_path(), &Some("inline".into()), &None)
                .await
                .unwrap(),
            "inline"
        );

        // The runner does not get to assume it was validated: a traversal path
        // is refused HERE, not merely upstream. `POST /workflows/{id}/test`
        // reaches dispatch without `validate_for_install`.
        let err = load_raw_prompt(
            "llm_1",
            root.as_path(),
            &None,
            &Some("prompts/../prompts/real.md".into()),
        )
        .await
        .expect_err("a '..' path must be refused by the runner itself");
        assert!(err.contains("must be a bundle-relative path"), "{err}");

        let err = load_raw_prompt("llm_1", root.as_path(), &None, &Some("/etc/passwd".into()))
            .await
            .expect_err("an absolute path must be refused by the runner itself");
        assert!(err.contains("must be a bundle-relative path"), "{err}");

        // An INTERMEDIATE symlink out of the bundle is refused. A STATIC fixture
        // like this one is caught by canonicalize+confine too — what it pins is
        // that the kernel-confined path agrees. The case only the single
        // `openat2` resolution can catch is the RACING one (a directory swapped
        // for a symlink between the check and the open), which a unit test cannot
        // construct deterministically; the anchor half of that attack IS pinned,
        // by `read_prompt_file_refuses_a_bundle_root_that_became_a_symlink`.
        #[cfg(unix)]
        {
            let err = load_raw_prompt(
                "llm_1",
                root.as_path(),
                &None,
                &Some("escapedir/outside.md".into()),
            )
            .await
            .expect_err("a path through a symlinked directory must not resolve");
            assert!(err.contains("outside bundle"), "{err}");
        }

        // A zero-byte prompt file is not a prompt — symmetric with `prompt: ""`.
        let err = load_raw_prompt("llm_1", root.as_path(), &None, &Some("prompts/empty.md".into()))
            .await
            .expect_err("a zero-byte prompt_file must not ship an empty prompt to the model");
        assert!(err.contains("is empty"), "{err}");
    }
}
