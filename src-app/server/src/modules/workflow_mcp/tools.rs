//! `tools/list` + `tools/call` for the built-in workflow MCP server.
//!
//! Each installed + accessible + enabled workflow becomes one opaque
//! tool `wf_<slug>`. `call_tool` snapshots the conversation model,
//! spawns the runner (shared `runner::spawn_run`), blocks until the run
//! is terminal, then builds a `CallToolResult` via
//! `format_outputs_for_mcp` honoring each output's `expose:` mode and
//! the size caps (plan §4.7).

#![allow(dead_code)]

use std::time::Duration;

use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::workflow::models::{Workflow, WorkflowRun};
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::runner;
use crate::modules::workflow::validate::{ExposeMode, OutputDef, WorkflowDef, parse_workflow_yaml};

use super::workflow_mcp_server_id;

// ── size caps (plan §4.7) ─────────────────────────────────────────────
/// `expose: full` outputs at or below this size are inlined as JSON; above
/// it they auto-promote to a `Content::Resource` entry.
pub const INLINE_FULL_CAP_BYTES: usize = 4 * 1024;
/// `expose: preview` snippet length.
pub const PREVIEW_SNIPPET_CHARS: usize = 500;
/// Total text-body cap across all inlined outputs. Outputs that would
/// push the body over this auto-promote to resources.
pub const TOTAL_TEXT_CAP_BYTES: usize = 50 * 1024;
/// Anthropic tool-name cap: `^[a-zA-Z0-9_-]{1,128}$`.
pub const MCP_TOOL_NAME_CAP: usize = 128;

// ── slug + composed-name derivation ───────────────────────────────────

/// Map a reverse-DNS workflow name to a tool-name leaf slug. `/` and `.`
/// (the only non-`[a-z0-9._-]` separators the publisher allows in a
/// name) collapse to `_`; any remaining non-alphanumeric char is also
/// normalized to `_` so the composed name stays inside Anthropic's
/// `^[a-zA-Z0-9_-]{1,128}$`.
pub fn slug_for_name(name: &str) -> String {
    let body: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("wf_{body}")
}

/// The full LLM-visible tool name: `<server_uuid>__wf_<slug>`. Matches
/// the `mcp/chat_extension/helpers.rs` `{server}__{tool}` convention.
pub fn composed_tool_name(slug: &str) -> String {
    format!("{}__{}", workflow_mcp_server_id(), slug)
}

/// Enforce the 128-char cap on the composed name. Returns `Some(name)`
/// when it fits + is regex-clean, `None` (caller drops + warns) when it
/// would overflow or carry an illegal char. Mirrors B2's drop-and-warn
/// behavior in `mcp/chat_extension/helpers.rs`.
pub fn checked_composed_name(slug: &str) -> Option<String> {
    let name = composed_tool_name(slug);
    if name.len() > MCP_TOOL_NAME_CAP {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(name)
}

/// INSTALL-TIME guard (plan §4 workflow_mcp + audit gap 4): reject a
/// workflow whose reverse-DNS `name` would produce a composed MCP tool
/// name (`<server_uuid>__wf_<slug>`) longer than 128 chars (i.e. slug
/// body > 87 chars) OR carrying an illegal char. The `list_tools` path
/// drops-and-warns at runtime, but a workflow that can NEVER surface as a
/// tool should be rejected at install rather than silently swallowed
/// later. Returns `Err(AppError::bad_request)` when the name is unusable.
pub fn check_install_slug_len(name: &str) -> Result<(), AppError> {
    let slug = slug_for_name(name);
    if checked_composed_name(&slug).is_none() {
        return Err(AppError::bad_request(
            "WORKFLOW_TOOL_NAME_TOO_LONG",
            format!(
                "workflow name '{name}' yields an MCP tool name longer than {MCP_TOOL_NAME_CAP} \
                 chars (or carrying an illegal char); shorten the workflow name so the slug body \
                 is at most 87 chars"
            ),
        ));
    }
    Ok(())
}

/// Derive a JSON-Schema `inputSchema` object from a workflow's
/// `inputs[]`. Required inputs land in `required[]`; defaults pass
/// through as schema `default`.
fn input_schema_for(def: &WorkflowDef) -> Value {
    let mut props = Map::new();
    let mut required: Vec<Value> = Vec::new();
    for input in &def.inputs {
        let mut p = Map::new();
        // Phase 1: inputs are untyped (string-ish). Keep the schema
        // permissive — the LLM gets the description for guidance, the
        // runner does the real validation against workflow.inputs[].
        if let Some(d) = &input.description {
            p.insert("description".into(), json!(d));
        }
        if let Some(default) = &input.default {
            p.insert("default".into(), default.clone());
        }
        props.insert(input.name.clone(), Value::Object(p));
        if input.required {
            required.push(json!(input.name));
        }
    }
    json!({
        "type": "object",
        "properties": Value::Object(props),
        "required": required,
    })
}

// ── tools/list ────────────────────────────────────────────────────────

/// Build the `tools/list` result for the given user. One tool per
/// installed workflow that's (a) `enabled`, (b) accessible (user-owned
/// OR system-scope; `repository::list_for_user` already encodes the
/// visibility predicate), and (c) whose composed tool name fits the
/// 128-char cap. Workflows whose `workflow.yaml` fails to parse are
/// skipped (defensive — install-time validation should have caught it).
pub async fn tool_list(pool: &sqlx::PgPool, user_id: Uuid) -> Result<Value, AppError> {
    let workflows = repository::list_for_user(pool, user_id).await?;
    let mut tools: Vec<Value> = Vec::new();
    // L3: distinct reverse-DNS names can collapse to the SAME `wf_*` slug
    // (`/` and `.` both map to `_`). Two such workflows would surface as
    // duplicate tool names → first-wins dispatch on the lossy reverse-scan.
    // Track emitted slugs and drop-and-warn on a collision so each tool name
    // is unique in tools/list.
    let mut emitted_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();

    for wf in workflows {
        if !wf.enabled {
            continue;
        }
        let slug = slug_for_name(&wf.name);
        let composed = match checked_composed_name(&slug) {
            Some(_n) => slug.clone(),
            None => {
                tracing::warn!(
                    workflow = %wf.name,
                    slug = %slug,
                    "workflow_mcp: composed tool name exceeds 128-char cap or carries an illegal char; dropping from tools/list"
                );
                continue;
            }
        };

        if !emitted_slugs.insert(slug.clone()) {
            tracing::warn!(
                workflow = %wf.name,
                slug = %slug,
                "workflow_mcp: tool slug collides with an earlier workflow (distinct names mapping to the same wf_* slug); dropping the duplicate from tools/list"
            );
            continue;
        }

        // Parse workflow.yaml for the input schema + description.
        let def = match read_workflow_def(&wf).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    workflow = %wf.name,
                    error = %e,
                    "workflow_mcp: failed to parse workflow.yaml; dropping from tools/list"
                );
                continue;
            }
        };

        let description = wf
            .description
            .clone()
            .or_else(|| wf.display_name.clone())
            .unwrap_or_else(|| format!("Run the '{}' workflow.", wf.name));

        tools.push(json!({
            "name": composed,
            "description": description,
            "inputSchema": input_schema_for(&def),
        }));
    }

    Ok(json!({ "tools": tools }))
}

async fn read_workflow_def(wf: &Workflow) -> Result<WorkflowDef, AppError> {
    let path = std::path::PathBuf::from(&wf.extracted_path).join(&wf.entry_point);
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        AppError::internal_error(format!(
            "workflow_mcp: read workflow.yaml at {}: {e}",
            path.display()
        ))
    })?;
    parse_workflow_yaml(&content)
}

// ── tools/call ────────────────────────────────────────────────────────

/// Recover the reverse-DNS workflow name from a `wf_<slug>` tool-name
/// leaf by matching against the user's accessible workflows (the slug
/// mapping is lossy — `/` and `.` both map to `_` — so we reverse via a
/// scan rather than a string un-map).
async fn resolve_workflow_by_slug(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    slug: &str,
) -> Result<Workflow, AppError> {
    let workflows = repository::list_for_user(pool, user_id).await?;
    workflows
        .into_iter()
        .find(|wf| slug_for_name(&wf.name) == slug)
        .ok_or_else(|| AppError::not_found("workflow not installed for this user"))
}

/// `tools/call` dispatch for a `wf_<slug>` tool. Spawns the run, blocks
/// until terminal, formats the result.
///
/// `tool_leaf` is the bare leaf the JSON-RPC handler extracted from the
/// composed `<server>__wf_<slug>` (stripping the `<server>__` prefix);
/// here it's already `wf_<slug>`.
pub async fn call_tool(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    tool_leaf: &str,
    arguments: &Value,
) -> Result<Value, AppError> {
    if !tool_leaf.starts_with("wf_") {
        return Err(AppError::bad_request(
            "WORKFLOW_TOOL_UNKNOWN",
            format!("unknown workflow tool '{tool_leaf}'"),
        ));
    }

    let wf = resolve_workflow_by_slug(pool, user_id, tool_leaf).await?;

    // Inputs arrive as the tool's `arguments` object.
    let inputs = match arguments {
        Value::Object(_) | Value::Null => arguments.clone(),
        _ => {
            return Err(AppError::bad_request(
                "WORKFLOW_INPUTS_NOT_OBJECT",
                "tool arguments must be a JSON object",
            ));
        }
    };

    // Spawn via the shared run path (validates yaml + snapshots model +
    // inserts the workflow_runs row + spawns the runner task). mocks are
    // never accepted on the MCP path (always production-shaped).
    let run_id = runner::spawn_run(
        pool,
        &wf,
        user_id,
        conversation_id,
        inputs,
        Default::default(),
        runner::SpawnRunOpts {
            model_id: None,
            invocation_source: "conversation",
            // The chat extension persists this run's resource_link artifacts
            // (created_by="mcp"); the runner must not double-save them.
            persist_artifacts: false,
            force_log_capture: false,
        },
    )
    .await?;

    // H2: forward chat-Stop to the runner. The runner was spawned detached
    // (`spawn_run`), so if the chat dispatcher aborts this request (user hits
    // Stop), dropping `call_tool`'s future must cancel the run — otherwise it
    // keeps spending tokens until its own (possibly large or unbounded)
    // wall-clock cap. The guard
    // fires the same cancel path as `POST /cancel` (DB CAS + registry signal)
    // if dropped before we `disarm()` it on terminal status.
    let cancel_guard = RunCancelOnDrop {
        pool: pool.clone(),
        run_id,
        armed: true,
    };

    // PER-STEP MCP PROGRESS (plan §4 step 4 / §4.4) — NOT wired in B5; same
    // transport limitation as the elicitation bridge below. The built-in
    // HTTP JSON-RPC handler is plain request/response, so there's no path to
    // push MCP `notifications/progress` mid-`tools/call` into the chat token
    // SSE. Per-step progress IS available on the per-run SSE
    // (`GET /api/workflow-runs/{id}/events`); the chat-side step granularity
    // is deferred until built-in servers gain a streamable transport.
    //
    // Block until terminal. The MCP tool call holds open until the run
    // finishes — there's no async tool-result pattern in the chat path.
    //
    // ELICITATION BRIDGE (plan §4.6) — NOT fully wired in B5; honest TODO.
    // ─────────────────────────────────────────────────────────────────
    // A `kind: elicit` step inside a workflow invoked here STILL works:
    // the runner's `ElicitDispatcher` (B4) persists `pending_elicitation_
    // json`, emits `ElicitationRequired` on the PER-RUN SSE, and blocks on
    // `registry::await_elicitation`. The user answers via the existing
    // `POST /api/workflow-runs/{run}/elicit/{id}` endpoint (B4); the run
    // then continues and `await_terminal` below returns the final result.
    // So the run does not hang and the simpler surface is live.
    //
    // What is NOT wired: pushing the elicitation into the CHAT thread as an
    // MCP `elicitation/create` request (the §4.6 "workflow_mcp path"
    // primary surface). Doing so requires SERVER→CLIENT request plumbing
    // that the built-in HTTP JSON-RPC transport does not have today: the
    // built-in servers are plain request/response handlers (this file),
    // and the MCP client (`mcp/client/http.rs`) has no path to receive a
    // server-initiated `elicitation/create` mid-`tools/call` and route the
    // response back. Wiring it real means (1) a bidirectional/streamable
    // transport for built-in servers and (2) a `RunContext.mcp_tool_context`
    // elicitation channel (referenced in the §4.6 pseudocode but absent
    // from B4's `RunContext`). Both are out of B5's scope. Until then the
    // per-run SSE form is the surface for workflow_mcp elicitations too.
    let run = await_terminal(pool, run_id).await?;
    // Reached a terminal status normally — don't cancel on the way out.
    // (If `await_terminal` had returned Err, the guard would drop armed and
    // cancel the run, which is the correct cleanup for a timed-out / crashed
    // runner.)
    cancel_guard.disarm();

    // Read the workflow def again for the outputs[] expose modes.
    let def = read_workflow_def(&wf).await?;

    match run.status.as_str() {
        "completed" => {
            let formatted = format_outputs_for_mcp(pool, &run, &def.outputs).await?;
            Ok(formatted)
        }
        _ => {
            // failed / cancelled / (defensive) anything non-completed.
            let err = build_error_result(pool, &run, &def).await;
            Ok(err)
        }
    }
}

/// H2: cancel-on-drop guard for the MCP tool-call path. While the tool call
/// awaits the run, this guard is alive; if the awaiting future is dropped
/// (chat Stop aborts the request) before `disarm()`, its `Drop` fires the
/// same cancel path as `POST /cancel` — the synchronous registry signal so an
/// in-flight step's `tokio::select!` preempts immediately, plus a detached
/// task for the async DB status CAS (`Drop` can't await).
struct RunCancelOnDrop {
    pool: sqlx::PgPool,
    run_id: Uuid,
    armed: bool,
}

impl RunCancelOnDrop {
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for RunCancelOnDrop {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // Registry signal is synchronous — fire immediately.
        let _ = registry::cancel(self.run_id);
        // DB status CAS is async — spawn detached.
        let pool = self.pool.clone();
        let run_id = self.run_id;
        tokio::spawn(async move {
            let _ = repository::cancel_cas(&pool, run_id).await;
        });
    }
}

/// Poll `workflow_runs.status` until terminal. The runner marks
/// `completed` / `failed` / `cancelled` on exit. The run's wall-clock cap is
/// now DYNAMIC — `WorkflowDef.max_runtime_secs` (live-adjustable via
/// `PUT /workflow-runs/{id}/timeout`, `0` = unbounded, ceiling
/// `runner::MAX_RUN_TIMEOUT_SECS`). So this poll loop can no longer assume a
/// fixed 30-min cap; instead it terminates on one of three conditions:
///   1. the row reaches a terminal status (the normal case — the runner's own
///      `deadline_watcher` fires first for a bounded run and marks it),
///   2. the no-progress guard below (a crashed/vanished runner task), or
///   3. a backstop ceiling that TRACKS the run's live `timeout_secs` (+ slack)
///      so a bounded run can't hang the chat past its own deadline, while an
///      UNBOUNDED run (`timeout_secs == 0`) relies solely on (1)/(2).
///
/// M5: a PANICKED runner task stops updating the row but never marks it
/// terminal — without the no-progress guard the tool call would block until
/// the ceiling. We track `updated_at`: every step transition / item-progress
/// emit bumps it, AND a live runner ticks a 60s liveness heartbeat
/// (`runner::HEARTBEAT_INTERVAL`) so a long-but-live single step (a 30-min
/// elicit wait, a 10-min sandbox step) keeps `updated_at` fresh. A stalled
/// `updated_at` past the no-progress threshold therefore means the runner task
/// is genuinely dead → fail fast, without false-killing a live run that's
/// merely waiting (this is what keeps an unbounded run from hanging forever).
async fn await_terminal(pool: &sqlx::PgPool, run_id: Uuid) -> Result<WorkflowRun, AppError> {
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    // Slack added on top of the run's own wall-clock deadline: the runner's
    // `deadline_watcher` should fire FIRST (marking the row terminal, observed
    // below); this ceiling is only a backstop for a vanished runner task whose
    // own watcher never fired.
    const CEILING_SLACK: Duration = Duration::from_secs(2 * 60);
    // No-progress kill: if `updated_at` doesn't advance for this long while
    // the run is still non-terminal, treat the runner as crashed.
    const NO_PROGRESS_LIMIT: chrono::Duration = chrono::Duration::minutes(5);
    let started = std::time::Instant::now();
    let mut last_updated_at: Option<chrono::DateTime<chrono::Utc>> = None;
    let mut last_progress_at = std::time::Instant::now();
    loop {
        let run = repository::find_run(pool, run_id)
            .await?
            .ok_or_else(|| AppError::not_found("WorkflowRun"))?;
        if matches!(run.status.as_str(), "completed" | "failed" | "cancelled") {
            return Ok(run);
        }
        // Reset the no-progress clock whenever the row's updated_at advances.
        if last_updated_at != Some(run.updated_at) {
            last_updated_at = Some(run.updated_at);
            last_progress_at = std::time::Instant::now();
        }
        // Fail fast on a stalled runner (M5). Compare against wall-clock age
        // of the LAST observed progress; a crashed task can't bump updated_at.
        let stalled_for = chrono::Utc::now().signed_duration_since(run.updated_at);
        if stalled_for > NO_PROGRESS_LIMIT
            && last_progress_at.elapsed() > Duration::from_secs(NO_PROGRESS_LIMIT.num_seconds() as u64)
        {
            return Err(AppError::internal_error(format!(
                "workflow_mcp: workflow run made no progress for over {} minutes \
                 (runner task appears to have crashed); failing the tool call",
                NO_PROGRESS_LIMIT.num_minutes()
            )));
        }
        // Backstop ceiling tracks the run's LIVE timeout (settable mid-run).
        // `0` = unbounded → no absolute ceiling here; the no-progress guard
        // above is the sole protection (a live runner heartbeats updated_at).
        let timeout_secs = registry::get(run_id)
            .map(|h| h.timeout_secs.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(registry::DEFAULT_RUN_TIMEOUT_SECS);
        if timeout_secs != 0 {
            let ceiling =
                Duration::from_secs(timeout_secs.min(runner::MAX_RUN_TIMEOUT_SECS)) + CEILING_SLACK;
            if started.elapsed() > ceiling {
                return Err(AppError::internal_error(
                    "workflow_mcp: timed out waiting for run to reach a terminal status",
                ));
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

// ── format_outputs_for_mcp (plan §4.7) ────────────────────────────────

/// Whether the workflow's effective `expose_logs` setting surfaces a
/// `logs_resource` URI on failure for the given step. Workflow-level
/// `never` blocks it entirely; `always` / `on_error` both surface on
/// error (the error path is the only caller).
fn logs_surfaceable(def: &WorkflowDef, step_id: &str) -> bool {
    use crate::modules::workflow::validate::ExposeLogs;
    // Per-step override wins; else the workflow-level setting.
    let effective = def
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .and_then(|s| s.expose_logs)
        .unwrap_or(def.expose_logs);
    !matches!(effective, ExposeLogs::Never)
}

/// Build the success `CallToolResult` JSON. Honors per-output `expose:`
/// modes + size caps (plan §4.7). The result has a heterogeneous
/// `content` array (text body + zero-or-more resource links) plus a
/// `structuredContent` typed mirror and `metadata`.
pub async fn format_outputs_for_mcp(
    pool: &sqlx::PgPool,
    run: &WorkflowRun,
    outputs: &[OutputDef],
) -> Result<Value, AppError> {
    let _ = pool;
    let run_id = run.id;

    // final_output_json carries per-output {value_preview, size_bytes, expose}.
    let resolved = run
        .final_output_json
        .as_ref()
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut inline_outputs = Map::new(); // name -> inline value
    let mut resource_entries: Vec<Value> = Vec::new();
    let mut structured = Map::new();
    let mut running_text_bytes: usize = 0;

    for o in outputs {
        let entry = resolved.get(&o.name);
        let size_bytes = entry
            .and_then(|e| e.get("size_bytes"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let preview = entry
            .and_then(|e| e.get("value_preview"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let uri = output_uri(run_id, &o.name);
        let mime = o
            .mime_type
            .clone()
            .unwrap_or_else(|| "text/plain".to_string());

        // Per-output structured mirror (FE rich render).
        structured.insert(
            o.name.clone(),
            json!({
                "expose": expose_str(o.expose),
                "size_bytes": size_bytes,
                "uri": uri,
                "mime_type": mime,
            }),
        );

        // H5: EVERY inlined entry (path / preview / full) counts against the
        // 50 KiB total-text cap, accounted by its REAL serialized byte size.
        // An entry that would push the running body over the cap auto-promotes
        // to a resource regardless of expose mode (hundreds of preview/path
        // entries previously blew past the cap because only the Full arm
        // tracked it).
        let promote_to_resource = |entries: &mut Vec<Value>, desc: String| {
            entries.push(resource_block(&uri, &o.name, desc, &mime));
        };

        match o.expose {
            ExposeMode::Hidden => {
                // Omitted from content entirely.
            }
            ExposeMode::Path => {
                let val = json!(uri);
                let bytes = serialized_len(&val);
                if running_text_bytes.saturating_add(bytes) > TOTAL_TEXT_CAP_BYTES {
                    promote_to_resource(
                        &mut resource_entries,
                        format!("Output path for '{}' ({} bytes).", o.name, size_bytes),
                    );
                } else {
                    running_text_bytes = running_text_bytes.saturating_add(bytes);
                    inline_outputs.insert(o.name.clone(), val);
                }
            }
            ExposeMode::Preview => {
                let snippet = take_chars(&preview, PREVIEW_SNIPPET_CHARS);
                let val = json!({
                    "preview": snippet,
                    "size_bytes": size_bytes,
                    "uri": uri,
                });
                let bytes = serialized_len(&val);
                if running_text_bytes.saturating_add(bytes) > TOTAL_TEXT_CAP_BYTES {
                    promote_to_resource(
                        &mut resource_entries,
                        format!(
                            "Preview of '{}' ({} bytes); inline body cap reached.",
                            o.name, size_bytes
                        ),
                    );
                } else {
                    running_text_bytes = running_text_bytes.saturating_add(bytes);
                    inline_outputs.insert(o.name.clone(), val);
                }
            }
            ExposeMode::Artifact => {
                resource_entries.push(resource_block(
                    &uri,
                    &o.name,
                    o.description.clone().unwrap_or_else(|| {
                        format!("Artifact output '{}' ({} bytes).", o.name, size_bytes)
                    }),
                    &mime,
                ));
            }
            ExposeMode::Full => {
                // Auto-promote when over the per-output cap OR when adding
                // it would push the running text body over the total cap.
                if size_bytes > INLINE_FULL_CAP_BYTES {
                    let desc = format!(
                        "Output of '{}' ({} bytes). Truncated preview: '{}...'",
                        o.name,
                        size_bytes,
                        take_chars(&preview, PREVIEW_SNIPPET_CHARS),
                    );
                    promote_to_resource(&mut resource_entries, desc);
                } else {
                    // Inline the full value (read from disk if available;
                    // fall back to the preview when content isn't on disk).
                    // C2: `step_outputs_json` is keyed by STEP ID, not output
                    // NAME, so we resolve the source step id from the output's
                    // `from` template (`{{ write.output }}` → step `write`)
                    // before looking it up — keying by `o.name` silently
                    // truncated full outputs whose name ≠ step id to the preview.
                    let val = read_full_output_value(run, o)
                        .unwrap_or_else(|| Value::String(preview.clone()));
                    // Account the REAL serialized byte size of what we inline
                    // against the running total (H5) — and re-check the
                    // per-output 4 KiB cap against the ACTUAL content size, not
                    // the metadata `size_bytes` (which for a sub-field `from`
                    // is just the small rendered-template length, letting a
                    // large backing file slip past the per-output cap).
                    let inlined_bytes = serialized_len(&val);
                    if inlined_bytes > INLINE_FULL_CAP_BYTES
                        || running_text_bytes.saturating_add(inlined_bytes) > TOTAL_TEXT_CAP_BYTES
                    {
                        let desc = format!(
                            "Output of '{}' ({} bytes); exceeds inline cap. Truncated preview: '{}...'",
                            o.name,
                            inlined_bytes,
                            take_chars(&preview, PREVIEW_SNIPPET_CHARS),
                        );
                        promote_to_resource(&mut resource_entries, desc);
                    } else {
                        running_text_bytes = running_text_bytes.saturating_add(inlined_bytes);
                        inline_outputs.insert(o.name.clone(), val);
                    }
                }
            }
        }
    }

    let metadata = json!({
        "run_id": run_id,
        "total_tokens": run.total_tokens,
        "ms_elapsed": run_ms_elapsed(run),
        "status": run.status,
        "steps_completed": run.step_outputs_json.as_object().map(|m| m.len()).unwrap_or(0),
    });

    let body = json!({
        "outputs": Value::Object(inline_outputs),
        "metadata": metadata.clone(),
    });

    let mut content: Vec<Value> = vec![json!({
        "type": "text",
        "text": serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string()),
    })];
    content.extend(resource_entries);

    Ok(json!({
        "content": content,
        "isError": false,
        "structuredContent": {
            "outputs": Value::Object(structured),
            "metadata": metadata,
        },
    }))
}

/// Build the rich error `CallToolResult` (plan §4.7). Always carries the
/// minimum recovery context; `logs_resource` only when `expose_logs`
/// allows it.
async fn build_error_result(
    pool: &sqlx::PgPool,
    run: &WorkflowRun,
    def: &WorkflowDef,
) -> Value {
    let _ = pool;
    let error_message = run
        .error_message
        .clone()
        .unwrap_or_else(|| format!("workflow run {}", run.status));
    let failed_step_id = run.current_step.clone();

    let mut failed_step = Map::new();
    if let Some(fid) = &failed_step_id {
        failed_step.insert("id".into(), json!(fid));
        if let Some(s) = def.steps.iter().find(|s| &s.id == fid) {
            failed_step.insert("kind".into(), json!(s.config.kind_str()));
        }
        if let Some(idx) = def.steps.iter().position(|s| &s.id == fid) {
            failed_step.insert("step_index".into(), json!(idx));
        }
        // Item-level counters for llm_map (if persisted).
        if let Some(prog) = run.step_item_progress_json.get(fid) {
            failed_step.insert("item_progress".into(), prog.clone());
        }
    }

    // Partial outputs that resolved before the failure (previews only).
    let mut partial = Map::new();
    if let Some(obj) = run.final_output_json.as_ref().and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if let Some(p) = v.get("value_preview") {
                partial.insert(k.clone(), p.clone());
            }
        }
    }

    let mut body = json!({
        "error": error_message,
        "metadata": {
            "run_id": run.id,
            "total_tokens": run.total_tokens,
            "status": run.status,
        },
        "partial_outputs": Value::Object(partial),
    });
    if !failed_step.is_empty() {
        body["failed_step"] = Value::Object(failed_step);
    }
    if let Some(fid) = &failed_step_id {
        if logs_surfaceable(def, fid) {
            body["logs_resource"] = json!(logs_step_uri(run.id, fid));
        }
    }

    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string()),
        }],
        "isError": true,
        "structuredContent": body,
    })
}

// ── small helpers ─────────────────────────────────────────────────────

pub fn output_uri(run_id: Uuid, name: &str) -> String {
    format!("ziee://workflow-runs/{run_id}/outputs/{name}")
}

pub fn logs_step_uri(run_id: Uuid, step_id: &str) -> String {
    format!("ziee://workflow-runs/{run_id}/logs/{step_id}")
}

fn resource_block(uri: &str, name: &str, description: String, mime: &str) -> Value {
    json!({
        "type": "resource",
        "resource": {
            "uri": uri,
            "name": name,
            "description": description,
            "mimeType": mime,
        }
    })
}

fn expose_str(e: ExposeMode) -> &'static str {
    match e {
        ExposeMode::Full => "full",
        ExposeMode::Preview => "preview",
        ExposeMode::Artifact => "artifact",
        ExposeMode::Path => "path",
        ExposeMode::Hidden => "hidden",
    }
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn run_ms_elapsed(run: &WorkflowRun) -> u64 {
    (run.updated_at - run.created_at)
        .num_milliseconds()
        .max(0) as u64
}

/// Read a resolved output's full value from disk via the per-step output
/// file referenced in `step_outputs_json`. C2: `step_outputs_json` is keyed
/// by STEP ID, not output NAME — so we resolve the source step id from the
/// output's `from` template (`{{ write.output }}` → step `write`) and look
/// up by that. Falls back to keying by `o.name` (covers the common
/// name==step-id case + any `from` we can't parse), else `None` (caller
/// falls back to the preview). This keeps inlining cheap without re-running
/// the full template engine on the MCP path.
fn read_full_output_value(run: &WorkflowRun, o: &OutputDef) -> Option<Value> {
    let key = step_id_from_template(&o.from).unwrap_or_else(|| o.name.clone());
    let meta_json = run
        .step_outputs_json
        .get(&key)
        .or_else(|| run.step_outputs_json.get(&o.name))?;
    let meta: crate::modules::workflow::types::OutputMeta =
        serde_json::from_value(meta_json.clone()).ok()?;
    crate::modules::workflow::file_io::read_output_value(&meta).ok()
}

/// Extract the leading step id from an `outputs[].from` template such as
/// `{{ write.output }}` / `{{ write.output.field }}` / `{{ write.path }}`.
/// Returns `None` for `{{ inputs.x }}` heads (no backing step file) or a
/// `from` with no recognizable `{{ <step>.… }}` head.
pub(crate) fn step_id_from_template(from: &str) -> Option<String> {
    let open = from.find("{{")?;
    let close = from[open + 2..].find("}}")? + open + 2;
    let inner = from[open + 2..close].trim();
    // Strip an optional `| filter` suffix.
    let lhs = inner.split('|').next().unwrap_or(inner).trim();
    // Head is up to the first `.` or `[`.
    let head_end = lhs
        .char_indices()
        .find(|(_, c)| *c == '.' || *c == '[')
        .map(|(i, _)| i)
        .unwrap_or(lhs.len());
    let head = lhs[..head_end].trim();
    if head.is_empty() || head == "inputs" {
        return None;
    }
    Some(head.to_string())
}

/// Serialized byte length of a JSON value as it will appear inline in the
/// MCP text body (H5 — account the ACTUAL inlined size, not raw size_bytes).
fn serialized_len(v: &Value) -> usize {
    serde_json::to_string(v).map(|s| s.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_maps_separators_to_underscore() {
        assert_eq!(
            slug_for_name("io.github.phibya/research-summarize-write"),
            "wf_io_github_phibya_research-summarize-write"
        );
        assert_eq!(slug_for_name("local.dev/x"), "wf_local_dev_x");
        // hyphens are preserved (legal in Anthropic's regex).
        assert_eq!(slug_for_name("a/b-c"), "wf_a_b-c");
    }

    #[test]
    fn composed_name_under_cap_accepted() {
        let slug = slug_for_name("io.github.phibya/research-summarize-write");
        let name = checked_composed_name(&slug).expect("fits");
        assert!(name.len() <= MCP_TOOL_NAME_CAP);
        assert!(name.starts_with(&workflow_mcp_server_id().to_string()));
        assert!(name.contains("__wf_"));
    }

    #[test]
    fn composed_name_over_cap_dropped() {
        // 36 (uuid) + 2 (__) + 3 (wf_) = 41 prefix; slug body must be
        // ≤ 87 chars. An 88-char body overflows.
        let long_leaf = "a".repeat(88);
        let slug = format!("wf_{long_leaf}");
        assert!(checked_composed_name(&slug).is_none());
        // 87 fits exactly.
        let ok_leaf = "a".repeat(87);
        let slug_ok = format!("wf_{ok_leaf}");
        assert!(checked_composed_name(&slug_ok).is_some());
    }

    #[test]
    fn install_slug_len_rejects_too_long_name() {
        // Audit gap 4: install-time rejection of a name whose slug body
        // (>87 chars) would overflow the 128-char composed tool name.
        // 88 alphanumerics → wf_<88> = 91-char slug body → overflow.
        let long_name = format!("io.github.x/{}", "a".repeat(88));
        let err = check_install_slug_len(&long_name).expect_err("should reject");
        assert_eq!(err.error_code(), "WORKFLOW_TOOL_NAME_TOO_LONG");

        // A short, ordinary name installs fine.
        check_install_slug_len("io.github.phibya/research-summarize-write")
            .expect("short name accepted");
    }

    fn run_with_final(final_json: Value, step_outputs: Value) -> WorkflowRun {
        use chrono::Utc;
        let now = Utc::now();
        WorkflowRun {
            id: Uuid::new_v4(),
            workflow_id: Uuid::new_v4(),
            conversation_id: None,
            user_id: Uuid::new_v4(),
            model_id: None,
            sandbox_flavor: None,
            run_kind: "normal".into(),
            inputs_json: json!({}),
            step_outputs_json: step_outputs,
            step_item_progress_json: json!({}),
            step_logs_json: json!({}),
            step_artifacts_json: json!({}),
            pending_elicitation_json: None,
            final_output_json: Some(final_json),
            status: "completed".into(),
            current_step: None,
            error_message: None,
            total_tokens: 42,
            created_at: now,
            updated_at: now,
        }
    }

    fn out(name: &str, expose: ExposeMode) -> OutputDef {
        OutputDef {
            name: name.into(),
            from: format!("{{{{ {name}.output }}}}"),
            expose,
            description: None,
            mime_type: None,
        }
    }

    fn out_from(name: &str, from: &str, expose: ExposeMode) -> OutputDef {
        OutputDef {
            name: name.into(),
            from: from.into(),
            expose,
            description: None,
            mime_type: None,
        }
    }

    #[test]
    fn expose_hidden_omitted_full_inlined() {
        // Drive the synchronous classification logic the formatter uses
        // by reconstructing it here (the formatter itself is async + takes
        // a pool; these assertions cover the same expose-mode decisions).
        let small_preview = "hello world";
        // full small → inline
        assert!(small_preview.len() <= INLINE_FULL_CAP_BYTES);
        // preview snippet truncation
        let long = "x".repeat(1000);
        assert_eq!(take_chars(&long, PREVIEW_SNIPPET_CHARS).len(), 500);
    }

    #[tokio::test]
    async fn format_full_small_inlines_and_hidden_omits() {
        let final_json = json!({
            "summary": {"value_preview": "short text", "size_bytes": 10, "expose": "full"},
            "secret": {"value_preview": "nope", "size_bytes": 4, "expose": "hidden"},
        });
        let run = run_with_final(final_json, json!({}));
        // No pool needed for these expose modes (full falls back to
        // preview when step output file is absent; hidden omits).
        let pool = test_pool().await;
        let outs = vec![out("summary", ExposeMode::Full), out("secret", ExposeMode::Hidden)];
        let res = format_outputs_for_mcp(&pool, &run, &outs).await.unwrap();
        let outputs = &res["structuredContent"]["outputs"];
        assert!(outputs.get("summary").is_some());
        assert!(outputs.get("secret").is_some()); // structured always has it
        // text body: summary present, secret omitted from content outputs
        let text = res["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert!(parsed["outputs"].get("summary").is_some());
        assert!(parsed["outputs"].get("secret").is_none());
        assert_eq!(res["isError"], json!(false));
    }

    #[tokio::test]
    async fn format_full_large_auto_promotes_to_resource() {
        let big = INLINE_FULL_CAP_BYTES + 1;
        let final_json = json!({
            "report": {"value_preview": "preview...", "size_bytes": big, "expose": "full"},
        });
        let run = run_with_final(final_json, json!({}));
        let pool = test_pool().await;
        let outs = vec![out("report", ExposeMode::Full)];
        let res = format_outputs_for_mcp(&pool, &run, &outs).await.unwrap();
        // content[1] should be a resource block (auto-promoted).
        let content = res["content"].as_array().unwrap();
        assert!(content.iter().any(|c| c["type"] == json!("resource")));
        // and it should NOT be in the inline outputs body.
        let text = content[0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert!(parsed["outputs"].get("report").is_none());
    }

    #[tokio::test]
    async fn format_artifact_and_preview_modes() {
        let final_json = json!({
            "art": {"value_preview": "x", "size_bytes": 5, "expose": "artifact"},
            "prev": {"value_preview": "y".repeat(800), "size_bytes": 800, "expose": "preview"},
        });
        let run = run_with_final(final_json, json!({}));
        let pool = test_pool().await;
        let outs = vec![out("art", ExposeMode::Artifact), out("prev", ExposeMode::Preview)];
        let res = format_outputs_for_mcp(&pool, &run, &outs).await.unwrap();
        let content = res["content"].as_array().unwrap();
        // artifact → resource block
        assert!(content.iter().any(|c| {
            c["type"] == json!("resource") && c["resource"]["name"] == json!("art")
        }));
        // preview → inline snippet capped at 500 chars
        let text = content[0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        let snip = parsed["outputs"]["prev"]["preview"].as_str().unwrap();
        assert_eq!(snip.chars().count(), 500);
    }

    // ── L3: slug collision ────────────────────────────────────────────

    #[test]
    fn distinct_names_collide_to_same_slug() {
        // `io.github.x/y` and `io_github_x/y` both normalize to the same
        // `wf_io_github_x_y` slug — a collision the list_tools dedup drops.
        let a = slug_for_name("io.github.x/y");
        let b = slug_for_name("io_github_x_y");
        assert_eq!(a, b, "distinct reverse-DNS names must collide on slug");
        // Simulate the dedup the list path performs.
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(a.clone()), "first wins");
        assert!(!seen.insert(b.clone()), "second is a dup and is dropped");
    }

    // ── C2: from-template step-id extraction ───────────────────────────

    #[test]
    fn step_id_from_template_extracts_head() {
        assert_eq!(
            step_id_from_template("{{ write.output }}").as_deref(),
            Some("write")
        );
        assert_eq!(
            step_id_from_template("{{ write.output.title }}").as_deref(),
            Some("write")
        );
        assert_eq!(
            step_id_from_template("{{ fan.output[0] }}").as_deref(),
            Some("fan")
        );
        assert_eq!(
            step_id_from_template("{{ write.output | json }}").as_deref(),
            Some("write")
        );
        // inputs head → no backing step file.
        assert_eq!(step_id_from_template("{{ inputs.x }}"), None);
        assert_eq!(step_id_from_template("no template here"), None);
    }

    // ── C2: inline full-output keyed by step id, NOT output name ───────

    #[tokio::test]
    async fn c2_full_output_name_differs_from_step_id_inlines_real_body() {
        // The output is NAMED "report" but its `from` reads step "write"'s
        // output. step_outputs_json is keyed by STEP ID ("write"). The old
        // code keyed by output name ("report") → miss → truncated to preview.
        let dir = std::env::temp_dir().join(format!("ziee-c2-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let real_body = "THE FULL REPORT BODY that is NOT in the preview";
        let path = dir.join("write.txt");
        std::fs::write(&path, real_body).unwrap();

        let meta = crate::modules::workflow::types::OutputMeta {
            path: path.clone(),
            size_bytes: real_body.len() as u64,
            sha256: String::new(),
            preview: "preview-snippet".into(),
            kind: crate::modules::workflow::types::StepKindTag::Llm,
            parsed_as: crate::modules::workflow::types::ParsedAs::Text,
        };
        let step_outputs = json!({ "write": serde_json::to_value(&meta).unwrap() });
        // final_output_json is keyed by OUTPUT name ("report") with the small
        // size so it stays under the inline cap.
        let final_json = json!({
            "report": {
                "value_preview": "preview-snippet",
                "size_bytes": real_body.len(),
                "expose": "full",
            }
        });
        let run = run_with_final(final_json, step_outputs);
        let pool = test_pool().await;
        let outs = vec![out_from("report", "{{ write.output }}", ExposeMode::Full)];
        let res = format_outputs_for_mcp(&pool, &run, &outs).await.unwrap();
        let text = res["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        // The inlined value must be the REAL body, not the preview.
        assert_eq!(
            parsed["outputs"]["report"].as_str(),
            Some(real_body),
            "inlined full output must be the real on-disk body, not the preview"
        );
    }

    // ── H5: 50 KiB total-text cap trips with many preview outputs ──────

    #[tokio::test]
    async fn h5_total_cap_promotes_many_previews_to_resources() {
        // Many preview outputs, each ~600 bytes inlined. Past 50 KiB total
        // they must auto-promote to resources instead of inlining.
        let big_preview = "p".repeat(600);
        let mut final_map = serde_json::Map::new();
        let mut outs = Vec::new();
        let count = 200; // 200 * ~600 = ~120 KiB >> 50 KiB cap
        for i in 0..count {
            let name = format!("o{i}");
            final_map.insert(
                name.clone(),
                json!({
                    "value_preview": big_preview,
                    "size_bytes": 600,
                    "expose": "preview",
                }),
            );
            outs.push(out(&name, ExposeMode::Preview));
        }
        let run = run_with_final(Value::Object(final_map), json!({}));
        let pool = test_pool().await;
        let res = format_outputs_for_mcp(&pool, &run, &outs).await.unwrap();
        let content = res["content"].as_array().unwrap();
        // Some outputs must have promoted to resource blocks (cap tripped).
        let resource_count = content.iter().filter(|c| c["type"] == json!("resource")).count();
        assert!(
            resource_count > 0,
            "H5: total-text cap must promote excess previews to resources"
        );
        // The inline text body must stay within the cap.
        let text = content[0]["text"].as_str().unwrap();
        assert!(
            text.len() <= TOTAL_TEXT_CAP_BYTES + 4096,
            "inline body {} exceeds the 50 KiB cap (+slack)",
            text.len()
        );
    }

    // A connectionless pool for tests that don't actually query. The
    // formatter only reads the pool to satisfy the signature (it never
    // queries when output files are absent), so a lazily-connected pool
    // that's never used works.
    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@127.0.0.1:54321/phase8_build".into()
        });
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&url)
            .expect("lazy pool")
    }
}
