//! Daemon↔pane JSON-RPC correlation broker (ITEM-9 pane path).
//!
//! The `/bridge` WSS carries JSON-RPC 2.0 in both directions. This module owns the
//! two pieces of shared state that let a `tools/call` handler (running in one task)
//! send a request to a connected Office task pane — whose socket is serviced in
//! another task — and await the correlated reply:
//!
//! - [`PANES`]   — registered task panes: `PaneId → { host, doc_key, tx }`.
//! - [`PENDING`] — in-flight daemon→pane requests: `corr_id → oneshot::Sender<…>`.
//!
//! Mirrors `bridge/auth.rs` (`LazyLock` + `Mutex`, poison-recovering) for the maps
//! and `mcp/elicitation/registry.rs` (register here, resolve from another task,
//! take-once) for the pending-correlation idiom. [`call_pane`] wraps the oneshot
//! recv in a wall-clock timeout (DEC-2).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use axum::extract::ws::Message;
use axum::http::StatusCode;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use ziee::AppError;

use super::protocol::{BridgeRequest, BridgeResponse, JSONRPC_VERSION};

/// Opaque per-connection pane identifier (monotonic, process-lifetime).
pub type PaneId = u64;

/// `call_pane` wall-clock timeout (DEC-2) — generous for an interactive Office.js
/// op on a large document.
const CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// Typed error codes surfaced to the MCP caller (mirrors the `OFFICE_*` codes in
/// `handlers.rs`; kept here so `call_pane` is self-contained).
pub const OFFICE_PANE_NOT_CONNECTED: &str = "OFFICE_PANE_NOT_CONNECTED";
pub const OFFICE_PANE_TIMEOUT: &str = "OFFICE_PANE_TIMEOUT";
pub const OFFICE_PANE_ERROR: &str = "OFFICE_PANE_ERROR";
/// Reused for a pane reply whose JSON-RPC error code is the pane's
/// "op unsupported on this host" signal (`-32002`), so a Word-only op invoked on
/// an Excel/PowerPoint pane surfaces the SAME code as the native PPT pre-gate.
pub const OFFICE_UNSUPPORTED_ON_HOST: &str = "OFFICE_UNSUPPORTED_ON_HOST";
/// The pane's JSON-RPC error code for "this op is not supported on this host"
/// (kept in sync with `resources/office-bridge/taskpane.js` `ERR_UNSUPPORTED_HOST`).
const PANE_ERR_UNSUPPORTED_HOST: i64 = -32002;

/// A pending daemon→pane request: the pane it was routed to (so a reply from a
/// DIFFERENT pane cannot resolve it — cross-pane spoofing guard) and the reply sink.
struct Pending {
    pane_id: PaneId,
    tx: oneshot::Sender<BridgeResponse>,
}

/// A connected task pane: the Office host it runs in, the document key it reported
/// at `register` time (DEC-1), and the sink that pushes frames to its socket.
struct PaneEntry {
    #[allow(dead_code)]
    host: String,
    doc_key: String,
    tx: mpsc::UnboundedSender<Message>,
}

static PANES: LazyLock<Mutex<HashMap<PaneId, PaneEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PENDING: LazyLock<Mutex<HashMap<u64, Pending>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PANE_IDS: AtomicU64 = AtomicU64::new(1);
static CORR_IDS: AtomicU64 = AtomicU64::new(1);

fn panes() -> MutexGuard<'static, HashMap<PaneId, PaneEntry>> {
    PANES.lock().unwrap_or_else(|p| p.into_inner())
}
fn pending() -> MutexGuard<'static, HashMap<u64, Pending>> {
    PENDING.lock().unwrap_or_else(|p| p.into_inner())
}

/// Allocate a fresh pane id (the socket loop calls this once per accepted socket).
pub fn next_pane_id() -> PaneId {
    PANE_IDS.fetch_add(1, Ordering::Relaxed)
}

/// Register a connected pane after its `register` hello. Overwrites any prior entry
/// for the same id (idempotent re-register).
pub fn register_pane(id: PaneId, host: String, doc_key: String, tx: mpsc::UnboundedSender<Message>) {
    panes().insert(id, PaneEntry { host, doc_key, tx });
}

/// Remove a pane on socket close, and fast-fail every in-flight request routed to
/// it: dropping the pending `oneshot::Sender` makes the waiting `call_pane`'s recv
/// error immediately (→ `OFFICE_PANE_NOT_CONNECTED`) instead of hanging to the
/// timeout. No-op if unknown.
pub fn unregister_pane(id: PaneId) {
    panes().remove(&id);
    let mut p = pending();
    let stale: Vec<u64> = p
        .iter()
        .filter(|(_, e)| e.pane_id == id)
        .map(|(corr, _)| *corr)
        .collect();
    for corr in stale {
        p.remove(&corr); // dropping the Sender fails the waiter fast
    }
}

/// Route a pane's response frame to the waiting [`call_pane`] — but ONLY if the
/// pending request was routed to THIS pane (`from_pane`). This binds each reply to
/// its originating pane so a second/compromised pane cannot resolve or forge another
/// pane's request by echoing a guessed (sequential) corr id. A response for an
/// unknown/stale corr id, or one from the wrong pane, is dropped (and logged).
pub fn route_response(from_pane: PaneId, resp: BridgeResponse) {
    let Some(corr) = resp.id.as_ref().and_then(Value::as_u64) else {
        tracing::debug!("office_bridge: pane {from_pane} response has no numeric id; dropping");
        return;
    };
    let mut p = pending();
    match p.get(&corr) {
        Some(entry) if entry.pane_id == from_pane => {
            let entry = p.remove(&corr).expect("just checked present");
            // The receiver may already be gone (timed out); ignore the send error.
            let _ = entry.tx.send(resp);
        }
        Some(_) => {
            tracing::warn!(
                "office_bridge: pane {from_pane} answered corr {corr} routed to a DIFFERENT pane; dropping (cross-pane response rejected)"
            );
        }
        None => {
            tracing::debug!("office_bridge: pane {from_pane} response for unknown/stale corr {corr}; dropping");
        }
    }
}

/// Which pane (if any) services `doc_full_name` (DEC-1). A pure function over
/// `(pane_id, doc_key)` candidates so it is unit-testable without the global map.
/// Ordered for SAFETY — a mutating op must never silently hit the wrong document:
/// 1. exact `doc_key` match wins (checked before basename so two docs sharing a
///    filename in different dirs route deterministically to the right pane);
/// 2. else a UNIQUE basename match (tolerates native-path vs Office.js `file://`
///    URL format differences); an ambiguous basename (≥2 matches) resolves to
///    NONE rather than guessing;
/// 3. else the sole connected pane ONLY when it reported an empty `doc_key` (an
///    unsaved document that cannot be matched by path); a sole pane with a known,
///    non-matching key is a DIFFERENT document, so it is never overridden.
fn resolve_pane(candidates: &[(PaneId, String)], doc_full_name: &str) -> Option<PaneId> {
    // 1. Exact full-path match.
    if let Some((id, _)) = candidates.iter().find(|(_, k)| k == doc_full_name) {
        return Some(*id);
    }
    // 2. Unique basename match among panes with a known key.
    let want = basename(doc_full_name);
    let mut bn = candidates
        .iter()
        .filter(|(_, k)| !k.is_empty() && basename(k) == want);
    if let Some((id, _)) = bn.next() {
        return if bn.next().is_none() { Some(*id) } else { None };
    }
    // 3. Sole pane, only if it is an unsaved doc (empty key).
    if candidates.len() == 1 && candidates[0].1.is_empty() {
        return Some(candidates[0].0);
    }
    None
}

/// Last path segment, tolerant of both `/` (posix / Office.js file URLs) and `\`
/// (Windows) separators.
fn basename(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// Send a JSON-RPC request to the pane serving `doc_full_name` and await the
/// correlated reply (DEC-1, DEC-2). Maps no-pane / pane-error / timeout to a typed
/// [`AppError`] the MCP dispatcher surfaces to the model.
pub async fn call_pane(doc_full_name: &str, method: &str, params: Value) -> Result<Value, AppError> {
    call_pane_with_timeout(doc_full_name, method, params, CALL_TIMEOUT).await
}

/// [`call_pane`] with an explicit timeout — the test seam (DEC-2) so deterministic
/// tests exercise the timeout path without sleeping the full 15s.
pub async fn call_pane_with_timeout(
    doc_full_name: &str,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, AppError> {
    // Resolve + clone the sink under the lock, then drop it before awaiting.
    let (pane_id, tx) = {
        let map = panes();
        let candidates: Vec<(PaneId, String)> =
            map.iter().map(|(id, e)| (*id, e.doc_key.clone())).collect();
        match resolve_pane(&candidates, doc_full_name) {
            Some(id) => map.get(&id).map(|e| (id, e.tx.clone())),
            None => None,
        }
    }
    .ok_or_else(|| not_connected_err(doc_full_name))?;

    let corr = CORR_IDS.fetch_add(1, Ordering::Relaxed);
    let (otx, orx) = oneshot::channel::<BridgeResponse>();
    pending().insert(corr, Pending { pane_id, tx: otx });

    let req = BridgeRequest {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id: Some(json!(corr)),
        method: method.to_string(),
        params: Some(params),
        session_token: None,
        host: None,
        doc_id: Some(doc_full_name.to_string()),
    };
    let frame = serde_json::to_string(&req)
        .map_err(|e| AppError::internal_error(format!("serialize bridge request: {e}")))?;
    if tx.send(Message::Text(frame.into())).is_err() {
        pending().remove(&corr);
        return Err(not_connected_err(doc_full_name));
    }

    match tokio::time::timeout(timeout, orx).await {
        // Pane answered. Enforce JSON-RPC result-XOR-error: an `error` maps to a
        // typed AppError (an "unsupported on this host" code is surfaced as the same
        // OFFICE_UNSUPPORTED_ON_HOST as the native PPT pre-gate); a `result` returns
        // it; NEITHER is a malformed reply and is rejected rather than coerced.
        Ok(Ok(resp)) => match (resp.error, resp.result) {
            (Some(err), _) => {
                let code = if err.code == PANE_ERR_UNSUPPORTED_HOST {
                    OFFICE_UNSUPPORTED_ON_HOST
                } else {
                    OFFICE_PANE_ERROR
                };
                Err(AppError::new(StatusCode::UNPROCESSABLE_ENTITY, code, err.message))
            }
            (None, Some(result)) => Ok(result),
            (None, None) => Err(AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                OFFICE_PANE_ERROR,
                "the Office task pane returned a reply with neither result nor error",
            )),
        },
        // Sender dropped — the socket closed mid-flight (see `unregister_pane`).
        Ok(Err(_)) => {
            pending().remove(&corr);
            Err(not_connected_err(doc_full_name))
        }
        // Timed out — drop the pending entry so it can't leak.
        Err(_) => {
            pending().remove(&corr);
            Err(AppError::new(
                StatusCode::GATEWAY_TIMEOUT,
                OFFICE_PANE_TIMEOUT,
                format!(
                    "the Office task pane for `{doc_full_name}` did not respond within {}s; \
                     confirm the document's ziee task pane is open and responsive (a very \
                     large document can also exceed the limit) and retry",
                    timeout.as_secs()
                ),
            ))
        }
    }
}

fn not_connected_err(doc: &str) -> AppError {
    AppError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        OFFICE_PANE_NOT_CONNECTED,
        format!(
            "no Office task pane is connected for `{doc}`; open the document's ziee task \
             pane (ribbon → Show Ziee Bridge) and retry"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// TEST-4 — pure pane resolution (DEC-1), SAFETY-ordered: exact wins over
    /// basename; ambiguous basename → none; sole-pane fallback only for an unsaved
    /// (empty-key) doc. No global state.
    #[test]
    fn resolve_pane_exact_then_unique_basename_then_empty_sole() {
        // Exact full-path match wins even when another pane shares the basename
        // (the two-docs-same-filename hazard must route deterministically).
        let collide = vec![
            (10u64, "/work/Report.docx".to_string()),
            (11u64, "/personal/Report.docx".to_string()),
        ];
        assert_eq!(resolve_pane(&collide, "/personal/Report.docx"), Some(11));
        assert_eq!(resolve_pane(&collide, "/work/Report.docx"), Some(10));
        // Ambiguous basename (no exact match) → none, never a guess.
        assert_eq!(resolve_pane(&collide, "/elsewhere/Report.docx"), None);

        // Unique basename match across differing path formats (native vs file URL).
        let fmt = vec![(20u64, "file:///Users/x/Report.docx".to_string())];
        assert_eq!(resolve_pane(&fmt, "/Users/x/Report.docx"), Some(20));

        // Sole pane with a KNOWN, non-matching key → none (it is a different doc).
        let known = vec![(30u64, "/Users/x/Other.docx".to_string())];
        assert_eq!(resolve_pane(&known, "/Users/x/Unknown.docx"), None);
        // Sole pane with an EMPTY key (unsaved doc) → the sole pane.
        let unsaved = vec![(31u64, String::new())];
        assert_eq!(resolve_pane(&unsaved, "Book1"), Some(31));

        // ≥2 panes, no match → none. Empty → none.
        assert_eq!(resolve_pane(&collide, "/x/Nope.docx"), None);
        assert_eq!(resolve_pane(&[], "/Users/x/Report.docx"), None);
    }

    /// TEST-1 — `call_pane` for a document with no matching pane →
    /// `OFFICE_PANE_NOT_CONNECTED`. Two decoy panes with unique, non-matching keys
    /// are registered so the result is deterministic regardless of any panes other
    /// (concurrent) tests hold: ≥2 panes with no key match defeats the sole-pane
    /// fallback (DEC-1), and a UUID-unique target basename can't collide.
    #[tokio::test]
    async fn call_pane_no_matching_pane_is_not_connected() {
        let (tx1, _rx1) = mpsc::unbounded_channel::<Message>();
        let (tx2, _rx2) = mpsc::unbounded_channel::<Message>();
        let d1 = next_pane_id();
        let d2 = next_pane_id();
        register_pane(d1, "word".into(), format!("/tmp/decoy-a-{d1}.docx"), tx1);
        register_pane(d2, "excel".into(), format!("/tmp/decoy-b-{d2}.xlsx"), tx2);

        let target = format!("/tmp/ziee-no-pane-{}-{}.docx", d1, d2);
        let err = call_pane(&target, "read_document", json!({}))
            .await
            .err()
            .expect("no matching pane → error");
        assert_eq!(err.error_code(), OFFICE_PANE_NOT_CONNECTED);

        unregister_pane(d1);
        unregister_pane(d2);
    }

    /// TEST-2 — register a pane against a mock receiver; `call_pane` pushes a
    /// well-formed `BridgeRequest` (method + numeric corr id + params), and
    /// `route_response` unblocks it with the result.
    #[tokio::test]
    async fn call_pane_pushes_request_and_route_response_returns_result() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = next_pane_id();
        let doc = format!("/tmp/ziee-test-doc-{id}.docx");
        register_pane(id, "word".to_string(), doc.clone(), tx);

        let doc2 = doc.clone();
        let handle = tokio::spawn(async move {
            call_pane(&doc2, "read_document", json!({ "doc_full_name": doc2 })).await
        });

        // The broker pushed a request frame; parse it.
        let msg = rx.recv().await.expect("broker pushed a frame");
        let text = match msg {
            Message::Text(t) => t,
            other => panic!("expected Text frame, got {other:?}"),
        };
        let req: BridgeRequest = serde_json::from_str(text.as_str()).expect("valid BridgeRequest");
        assert_eq!(req.method, "read_document");
        let corr = req.id.as_ref().and_then(Value::as_u64).expect("numeric corr id");

        // Answer it as THIS pane.
        route_response(
            id,
            BridgeResponse::ok(Some(json!(corr)), json!({ "text": "hello world" })),
        );

        let out = handle.await.unwrap().expect("call_pane resolves ok");
        assert_eq!(out, json!({ "text": "hello world" }));
        unregister_pane(id);
    }

    /// TEST-3 — a registered pane that never answers → `OFFICE_PANE_TIMEOUT`, and the
    /// pending entry is removed (no leak).
    #[tokio::test]
    async fn call_pane_times_out() {
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        let id = next_pane_id();
        let doc = format!("/tmp/ziee-test-timeout-{id}.docx");
        register_pane(id, "word".to_string(), doc.clone(), tx);

        let err = call_pane_with_timeout(&doc, "get_selection", json!({}), Duration::from_millis(50))
            .await
            .err()
            .expect("timeout → error");
        assert_eq!(err.error_code(), OFFICE_PANE_TIMEOUT);
        // (The timeout path removes its own pending entry — see `call_pane_with_timeout`;
        // asserting the global map's exact size here would be racy against concurrent
        // tests sharing the process-global PENDING map.)
        unregister_pane(id);
    }

    /// TEST-5 — dropping the pane's sink (socket closed) fails an in-flight
    /// `call_pane` with a typed error instead of hanging.
    #[tokio::test]
    async fn call_pane_fails_when_pane_sink_dropped() {
        let (tx, rx) = mpsc::unbounded_channel::<Message>();
        let id = next_pane_id();
        let doc = format!("/tmp/ziee-test-drop-{id}.docx");
        register_pane(id, "excel".to_string(), doc.clone(), tx);
        // Drop the receiver so the sink send fails immediately.
        drop(rx);

        let err = call_pane(&doc, "read_document", json!({}))
            .await
            .err()
            .expect("dropped sink → error");
        assert_eq!(err.error_code(), OFFICE_PANE_NOT_CONNECTED);
        unregister_pane(id);
    }

    /// A response from a DIFFERENT pane than the one a request was routed to must NOT
    /// resolve it (cross-pane spoofing guard); the legitimate pane's reply does.
    #[tokio::test]
    async fn route_response_rejects_wrong_pane() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let id = next_pane_id();
        let other = next_pane_id(); // a different, unrelated pane id
        let doc = format!("/tmp/ziee-xpane-{id}.docx");
        register_pane(id, "word".into(), doc.clone(), tx);

        let doc2 = doc.clone();
        let handle = tokio::spawn(async move { call_pane(&doc2, "read_document", json!({})).await });

        let msg = rx.recv().await.expect("request pushed");
        let text = match msg {
            Message::Text(t) => t,
            o => panic!("expected text, got {o:?}"),
        };
        let corr = serde_json::from_str::<BridgeRequest>(text.as_str())
            .unwrap()
            .id
            .and_then(|v| v.as_u64())
            .unwrap();

        // A forged reply from the WRONG pane is dropped (does not resolve the call).
        route_response(other, BridgeResponse::ok(Some(json!(corr)), json!({ "text": "FORGED" })));
        // The call is still pending; now the RIGHT pane answers.
        route_response(id, BridgeResponse::ok(Some(json!(corr)), json!({ "text": "REAL" })));

        let out = handle.await.unwrap().expect("resolves from the right pane");
        assert_eq!(out, json!({ "text": "REAL" }));
        unregister_pane(id);
    }

    /// Unregistering a pane fast-fails its in-flight `call_pane` (dropped oneshot →
    /// NOT_CONNECTED) instead of hanging to the timeout.
    #[tokio::test]
    async fn unregister_fast_fails_inflight_call() {
        let (tx, _rx) = mpsc::unbounded_channel::<Message>();
        let id = next_pane_id();
        let doc = format!("/tmp/ziee-unreg-{id}.docx");
        register_pane(id, "word".into(), doc.clone(), tx);

        let doc2 = doc.clone();
        // A generous timeout: if unregister did NOT fail it fast, this would block far
        // longer than the test's patience.
        let handle = tokio::spawn(async move {
            call_pane_with_timeout(&doc2, "read_document", json!({}), Duration::from_secs(30)).await
        });
        // Let the call register its pending entry, then unregister the pane.
        tokio::time::sleep(Duration::from_millis(50)).await;
        unregister_pane(id);

        let err = tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("call resolves quickly after unregister")
            .unwrap()
            .err()
            .expect("in-flight call fails on unregister");
        assert_eq!(err.error_code(), OFFICE_PANE_NOT_CONNECTED);
    }
}
