//! Streamable-HTTP (SSE) execution path for `execute_command`.
//!
//! `execute_command` is the one tool that can block on a multi-hundred-MB
//! rootfs download. To (a) ask the user for consent BEFORE that download
//! and (b) stream live progress, its `tools/call` responds with
//! `text/event-stream` instead of single-shot JSON — i.e. the built-in
//! code_sandbox server acts as a Streamable-HTTP MCP server for this one
//! tool. The chat's MCP client (`mcp/client/http.rs`) already routes on
//! the response Content-Type: a `text/event-stream` response is handed to
//! `call_tool_with_elicitation`, which handles `elicitation/create`,
//! forwards `notifications/progress`, and reads the final result.
//!
//! Wire flow over the SSE response:
//!   1. (optional) `elicitation/create` JSON-RPC *request* (string id) →
//!      the client prompts the user and POSTs the result back to
//!      `/code-sandbox` as `{jsonrpc, id, result:{action,...}}`, which
//!      [`try_resolve_elicitation`] routes to the awaiting task.
//!   2. (optional) `notifications/progress` while the rootfs downloads.
//!   3. the final `{jsonrpc, id, result:{content,isError,structuredContent}}`.
//!
//! The whole sequence runs in a spawned task that owns the per-conversation
//! lock + the elicitation oneshot; the SSE stream is a pure forwarder.

use std::time::Duration;

use axum::response::sse::{Event, Sse};
use dashmap::DashMap;
use futures_util::Stream;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::modules::code_sandbox::runtime_fetch::{self, FetchPhase};
use crate::modules::code_sandbox::types::{SandboxContext, KNOWN_FLAVORS};
use crate::modules::code_sandbox::{config, handlers, runtime_mount, tools};

/// How long the server waits for the user's elicitation response before
/// treating it as a cancel. Comfortably above the client's own 300s
/// elicitation budget; the chat turn is a detached background task so a
/// long think-time doesn't tie up the request thread.
const CONSENT_TIMEOUT_SECS: u64 = 600;

// =====================================================================
// Pure consent decision (Tier-1 testable)
// =====================================================================

/// Decide whether to prompt for download consent before fetching `flavor`.
///
/// Prompt **iff** consent is enabled AND the flavor isn't cached AND no
/// fetch is already in flight for it AND its advertised size is at/above
/// the auto-download threshold. A fetch already in flight means another
/// turn already approved + started the same download, so we silently join
/// it rather than re-prompting.
pub fn should_prompt_for_download(
    require_consent: bool,
    auto_download_under_mb: u64,
    flavor_size_mb: u64,
    cached: bool,
    fetch_in_flight: bool,
) -> bool {
    require_consent && !cached && !fetch_in_flight && flavor_size_mb >= auto_download_under_mb
}

// =====================================================================
// Server-side pending-elicitation registry
// =====================================================================
//
// Keyed by the string JSON-RPC id we put on each `elicitation/create`
// request. The client echoes that id verbatim when it POSTs the result
// back, so [`try_resolve_elicitation`] can hand it to the awaiting task.

static PENDING_ELICITATIONS: Lazy<DashMap<String, oneshot::Sender<Value>>> =
    Lazy::new(DashMap::new);

/// If `body` is a JSON-RPC *response* to a server-initiated elicitation
/// (a string `id` matching a pending request + a `result`/`error`, no
/// `method`), deliver the result to the awaiting execute stream and
/// return `true`. Returns `false` for anything else (e.g. real requests),
/// which the caller then handles as a normal JSON-RPC request.
pub fn try_resolve_elicitation(body: &Value) -> bool {
    if body.get("method").is_some() {
        return false; // it's a request, not a response
    }
    let id = match body.get("id").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return false, // our elicitation ids are always strings
    };
    if body.get("result").is_none() && body.get("error").is_none() {
        return false;
    }
    match PENDING_ELICITATIONS.remove(&id) {
        Some((_, tx)) => {
            // On a JSON-RPC error response, synthesize a cancel.
            let payload = body
                .get("result")
                .cloned()
                .unwrap_or_else(|| json!({ "action": "cancel" }));
            let _ = tx.send(payload);
            true
        }
        None => false,
    }
}

/// Handle an incoming `notifications/cancelled` for an in-flight streamed
/// call. Phase 1 parses + logs the requestId; Phase 2 wires the actual
/// abort of the in-flight `execute_command` task via the INFLIGHT registry.
///
/// We cancel ONLY on an explicit `notifications/cancelled` (user intent),
/// never on a mere SSE-stream drop — a page reload drops the stream but the
/// download should finish so a re-chat can reuse it (Plan 2 §4 resilience).
pub fn handle_cancelled(body: &Value) {
    let Some(request_id) = body.get("params").and_then(|p| p.get("requestId")) else {
        return;
    };
    let key = id_key(request_id);
    if let Some((_, handle)) = INFLIGHT.remove(&key) {
        // Aborting the task drops the bwrap `Child` (spawned with
        // `kill_on_drop(true)`, sandbox.rs) → SIGKILL — and drops the
        // per-conversation lock guard. An in-flight rootfs download running
        // on a `spawn_blocking` thread is detached and finishes harmlessly
        // (it just caches the file).
        handle.abort();
        tracing::info!(request_id = %key, "code_sandbox: cancelled in-flight execute_command");
    }
}

/// In-flight streamed `execute_command` tasks, keyed by the originating
/// `tools/call` JSON-RPC id, so a `notifications/cancelled` can abort one.
static INFLIGHT: Lazy<DashMap<String, tokio::task::AbortHandle>> = Lazy::new(DashMap::new);

/// Normalize a JSON-RPC id to a stable string key. A request id may be a
/// string or a number; `notifications/cancelled.requestId` echoes the same
/// value, so both sides key identically.
fn id_key(id: &Value) -> String {
    match id {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// =====================================================================
// Stream messages (rendered to SSE JSON-RPC frames)
// =====================================================================

enum StreamMsg {
    Elicit {
        id: String,
        message: String,
        schema: Value,
    },
    Progress {
        token: Option<Value>,
        progress: u32,
        total: u32,
        message: String,
    },
    Final {
        rpc_id: Value,
        result: Value,
    },
}

impl StreamMsg {
    /// Render to `(json-rpc frame, is_final)`.
    fn render(self) -> (Value, bool) {
        match self {
            StreamMsg::Elicit { id, message, schema } => (
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "elicitation/create",
                    "params": { "message": message, "requestedSchema": schema },
                }),
                false,
            ),
            StreamMsg::Progress { token, progress, total, message } => {
                let mut params = json!({ "progress": progress, "total": total, "message": message });
                if let Some(t) = token {
                    params["progressToken"] = t;
                }
                (
                    json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": params }),
                    false,
                )
            }
            StreamMsg::Final { rpc_id, result } => (
                json!({ "jsonrpc": "2.0", "id": rpc_id, "result": result }),
                true,
            ),
        }
    }
}

/// Coarse phase → (progress, total) mapping for `notifications/progress`.
/// Byte-granular download progress is a future enhancement (would require
/// instrumenting `download_to_file`); phases give a monotonic bar today.
fn phase_to_progress(phase: FetchPhase) -> (u32, u32) {
    let p = match phase {
        FetchPhase::Resolving => 1,
        FetchPhase::Downloading => 2,
        FetchPhase::VerifyingSha256 => 3,
        FetchPhase::VerifyingCosign => 4,
        FetchPhase::Installing => 5,
    };
    (p, 5)
}

// =====================================================================
// Public entry: build the SSE response for an execute_command call
// =====================================================================

pub fn execute_command_stream(
    ctx: SandboxContext,
    rpc_id: Value,
    command: String,
    flavor: String,
    progress_token: Option<Value>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, mut rx) = mpsc::unbounded_channel::<StreamMsg>();

    // Register the task under the tools/call id so `notifications/cancelled`
    // can abort it. The task removes its own entry on completion. (A benign
    // race where a fast task finishes before `insert` leaves a stale handle —
    // aborting a finished task is a harmless no-op.)
    let key = id_key(&rpc_id);
    let cleanup_key = key.clone();
    let join = tokio::spawn(async move {
        run_execute(ctx, rpc_id, command, flavor, progress_token, tx).await;
        INFLIGHT.remove(&cleanup_key);
    });
    INFLIGHT.insert(key, join.abort_handle());

    let stream = async_stream::stream! {
        while let Some(msg) = rx.recv().await {
            let (frame, is_final) = msg.render();
            yield Ok(Event::default().data(frame.to_string()));
            if is_final {
                break;
            }
        }
    };
    Sse::new(stream)
}

async fn run_execute(
    ctx: SandboxContext,
    rpc_id: Value,
    command: String,
    flavor: String,
    progress_token: Option<Value>,
    tx: mpsc::UnboundedSender<StreamMsg>,
) {
    let state = match config::get_state() {
        Some(s) => s,
        None => {
            let _ = tx.send(StreamMsg::Final {
                rpc_id,
                result: error_result("code_sandbox not initialized"),
            });
            return;
        }
    };

    // Per-conversation serialization, held for the whole streamed call.
    // NOTE: the elicitation-result POST is classified + resolved BEFORE
    // any lock acquisition in the handler, so it can never deadlock against
    // this guard while we await the user's consent.
    let lock = handlers::conv_lock(ctx.conversation_id);
    let _guard = lock.lock().await;

    // --- Consent gate --------------------------------------------------
    let cfg = &state.config;
    let size_mb = KNOWN_FLAVORS
        .iter()
        .find(|m| m.flavor == flavor)
        .map(|m| m.approximate_size_mb)
        .unwrap_or(0);
    let prompt = should_prompt_for_download(
        cfg.require_download_consent,
        cfg.auto_download_under_mb,
        size_mb,
        runtime_mount::is_flavor_cached(&state, &flavor),
        runtime_fetch::is_fetch_in_flight(&flavor),
    );

    if prompt {
        let id = format!("cs-elicit-{}", Uuid::new_v4());
        let (otx, orx) = oneshot::channel::<Value>();
        PENDING_ELICITATIONS.insert(id.clone(), otx);

        let message = format!(
            "Running this requires the '{flavor}' sandbox environment (~{size_mb} MB), \
             which isn't downloaded yet. Download it now? This is a one-time download."
        );
        // Empty-property object schema → the chat UI renders a plain
        // Accept/Decline confirm; the elicitation *action* is the answer.
        let schema = json!({ "type": "object", "properties": {}, "required": [] });
        let _ = tx.send(StreamMsg::Elicit { id: id.clone(), message, schema });

        let action = match tokio::time::timeout(Duration::from_secs(CONSENT_TIMEOUT_SECS), orx).await
        {
            Ok(Ok(v)) => v
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("cancel")
                .to_string(),
            _ => {
                // Timed out or sender dropped → treat as cancel; clean up.
                PENDING_ELICITATIONS.remove(&id);
                "cancel".to_string()
            }
        };

        if action != "accept" {
            let _ = tx.send(StreamMsg::Final {
                rpc_id,
                result: declined_result(&flavor, size_mb),
            });
            return;
        }
    }

    // --- Fetch with progress (if still uncached) -----------------------
    if !runtime_mount::is_flavor_cached(&state, &flavor) {
        let cache_dir = runtime_mount::cache_dir(&state);
        let txp = tx.clone();
        let token = progress_token.clone();
        let fetch_flavor = flavor.clone();
        let result = runtime_fetch::ensure_fetched(&cache_dir, &fetch_flavor, move |p| {
            let (progress, total) = phase_to_progress(p.phase);
            let _ = txp.send(StreamMsg::Progress {
                token: token.clone(),
                progress,
                total,
                message: p.message,
            });
        })
        .await;
        if let Err(e) = result {
            let _ = tx.send(StreamMsg::Final {
                rpc_id,
                result: error_result(&format!("rootfs download failed: {e}")),
            });
            return;
        }
    }

    // --- Execute (warm ensure_rootfs_ready + bwrap) --------------------
    match tools::execute::execute_command(&ctx, &command, &flavor).await {
        Ok(value) => {
            let content = handlers::mcp_content_blocks(&value);
            let _ = tx.send(StreamMsg::Final {
                rpc_id,
                result: json!({
                    "content": content,
                    "isError": false,
                    "structuredContent": value,
                }),
            });
        }
        Err(app_err) => {
            tracing::warn!(error = ?app_err, "code_sandbox: streamed execute_command failed");
            let _ = tx.send(StreamMsg::Final {
                rpc_id,
                result: json!({
                    "content": [{ "type": "text", "text": "execute_command failed" }],
                    "isError": true,
                }),
            });
        }
    }
}

fn declined_result(flavor: &str, size_mb: u64) -> Value {
    let text = format!(
        "The user declined to download the '{flavor}' sandbox environment (~{size_mb} MB), \
         so the command was not run. Let the user know, and ask whether they'd like to \
         proceed with the download or use a smaller environment."
    );
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
        "structuredContent": { "status": "download_declined", "flavor": flavor },
    })
}

fn error_result(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

// =====================================================================
// Tier 1 — unit tests for the consent decision
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_decision_table() {
        // (require, threshold, size, cached, in_flight) -> expected prompt
        let cases = [
            // Large + uncached + idle + consent on → prompt.
            ((true, 100, 853, false, false), true),
            // Cached → never prompt.
            ((true, 100, 853, true, false), false),
            // Already fetching → join, don't prompt.
            ((true, 100, 853, false, true), false),
            // Below threshold (minimal) → silent download.
            ((true, 100, 57, false, false), false),
            // Consent disabled → never prompt.
            ((false, 100, 853, false, false), false),
            // Exactly at threshold → prompt (>=).
            ((true, 100, 100, false, false), true),
        ];
        for ((require, threshold, size, cached, in_flight), expected) in cases {
            assert_eq!(
                should_prompt_for_download(require, threshold, size, cached, in_flight),
                expected,
                "case require={require} threshold={threshold} size={size} cached={cached} in_flight={in_flight}"
            );
        }
    }

    #[test]
    fn resolve_ignores_requests_and_unknown_ids() {
        // A request (has method) is never an elicitation response.
        assert!(!try_resolve_elicitation(&json!({
            "jsonrpc": "2.0", "id": "x", "method": "tools/call", "params": {}
        })));
        // A response with an unknown id → false (nothing pending).
        assert!(!try_resolve_elicitation(&json!({
            "jsonrpc": "2.0", "id": "cs-elicit-nope", "result": { "action": "accept" }
        })));
        // A message with neither result nor error → false.
        assert!(!try_resolve_elicitation(&json!({ "jsonrpc": "2.0", "id": "y" })));
    }

    #[tokio::test]
    async fn resolve_delivers_to_waiting_task() {
        let id = "cs-elicit-test-deliver".to_string();
        let (otx, orx) = oneshot::channel::<Value>();
        PENDING_ELICITATIONS.insert(id.clone(), otx);

        assert!(try_resolve_elicitation(&json!({
            "jsonrpc": "2.0", "id": id, "result": { "action": "accept", "content": {} }
        })));

        let got = orx.await.unwrap();
        assert_eq!(got.get("action").and_then(|a| a.as_str()), Some("accept"));
        // Entry consumed.
        assert!(!PENDING_ELICITATIONS.contains_key(&id));
    }

    #[test]
    fn id_key_normalizes_number_and_string() {
        assert_eq!(id_key(&json!(42)), "42");
        assert_eq!(id_key(&json!("42")), "42");
        assert_eq!(id_key(&json!("cs-elicit-x")), "cs-elicit-x");
    }

    // Phase 2: a `notifications/cancelled` for an in-flight call's id must
    // abort the registered task (which, in production, drops the bwrap Child
    // → SIGKILL via kill_on_drop, and releases the conversation lock).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn handle_cancelled_aborts_registered_task() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let id = json!(424242);
        let key = id_key(&id);

        let started = Arc::new(AtomicBool::new(false));
        let completed = Arc::new(AtomicBool::new(false));
        let s2 = started.clone();
        let c2 = completed.clone();
        let join = tokio::spawn(async move {
            s2.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(30)).await;
            c2.store(true, Ordering::SeqCst); // must NOT run once aborted
        });
        INFLIGHT.insert(key.clone(), join.abort_handle());

        // Let the task start.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(started.load(Ordering::SeqCst));

        handle_cancelled(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": id, "reason": "user navigated away" }
        }));

        // Registry entry consumed; task aborted (not run to completion).
        assert!(!INFLIGHT.contains_key(&key));
        let res = join.await;
        assert!(res.is_err() && res.unwrap_err().is_cancelled(), "task must be aborted");
        assert!(!completed.load(Ordering::SeqCst), "aborted task must not complete");
    }

    #[tokio::test]
    async fn handle_cancelled_unknown_id_is_noop() {
        // No registered task for this id → no panic, nothing happens.
        handle_cancelled(&json!({
            "method": "notifications/cancelled",
            "params": { "requestId": "no-such-id" }
        }));
    }
}
