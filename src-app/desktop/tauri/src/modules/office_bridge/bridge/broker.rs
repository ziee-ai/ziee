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
static PENDING: LazyLock<Mutex<HashMap<u64, oneshot::Sender<BridgeResponse>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PANE_IDS: AtomicU64 = AtomicU64::new(1);
static CORR_IDS: AtomicU64 = AtomicU64::new(1);

fn panes() -> MutexGuard<'static, HashMap<PaneId, PaneEntry>> {
    PANES.lock().unwrap_or_else(|p| p.into_inner())
}
fn pending() -> MutexGuard<'static, HashMap<u64, oneshot::Sender<BridgeResponse>>> {
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

/// Remove a pane on socket close. No-op if unknown.
pub fn unregister_pane(id: PaneId) {
    panes().remove(&id);
}

/// Route a pane's response frame to the waiting [`call_pane`] (if any). The socket
/// loop calls this for every inbound frame carrying `result`/`error`. A response for
/// an unknown/stale corr id is silently dropped.
pub fn route_response(resp: BridgeResponse) {
    if let Some(corr) = resp.id.as_ref().and_then(Value::as_u64)
        && let Some(tx) = pending().remove(&corr)
    {
        // The receiver may already be gone (timed out); ignore the send error.
        let _ = tx.send(resp);
    }
}

/// Which pane (if any) services `doc_full_name` (DEC-1). A pure function over
/// `(pane_id, doc_key)` candidates so it is unit-testable without the global map:
/// (a) exact `doc_key` match or shared basename, else (b) the sole connected pane,
/// else (c) none.
fn resolve_pane(candidates: &[(PaneId, String)], doc_full_name: &str) -> Option<PaneId> {
    let want = basename(doc_full_name);
    if let Some((id, _)) = candidates
        .iter()
        .find(|(_, k)| k == doc_full_name || basename(k) == want)
    {
        return Some(*id);
    }
    if candidates.len() == 1 {
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
    let tx = {
        let map = panes();
        let candidates: Vec<(PaneId, String)> =
            map.iter().map(|(id, e)| (*id, e.doc_key.clone())).collect();
        match resolve_pane(&candidates, doc_full_name) {
            Some(id) => map.get(&id).map(|e| e.tx.clone()),
            None => None,
        }
    }
    .ok_or_else(|| not_connected_err(doc_full_name))?;

    let corr = CORR_IDS.fetch_add(1, Ordering::Relaxed);
    let (otx, orx) = oneshot::channel::<BridgeResponse>();
    pending().insert(corr, otx);

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
        // Pane answered.
        Ok(Ok(resp)) => match resp.error {
            Some(err) => Err(AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                OFFICE_PANE_ERROR,
                err.message,
            )),
            None => Ok(resp.result.unwrap_or(Value::Null)),
        },
        // Sender dropped — the socket closed mid-flight.
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
                    "the Office task pane for `{doc_full_name}` did not respond within {}s",
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

    /// TEST-4 — pure pane resolution (DEC-1): exact/basename match wins; sole pane is
    /// the fallback; ≥2 panes with no match → none. No global state.
    #[test]
    fn resolve_pane_matches_then_sole_then_none() {
        // Exact full-name match.
        let c = vec![
            (10u64, "/Users/x/Report.docx".to_string()),
            (11u64, "/Users/x/Other.xlsx".to_string()),
        ];
        assert_eq!(resolve_pane(&c, "/Users/x/Report.docx"), Some(10));
        // Basename match across differing path formats (native path vs file URL).
        let c2 = vec![(20u64, "file:///Users/x/Report.docx".to_string())];
        assert_eq!(resolve_pane(&c2, "/Users/x/Report.docx"), Some(20));
        // Sole pane, no key match → the sole pane.
        let c3 = vec![(30u64, "whatever".to_string())];
        assert_eq!(resolve_pane(&c3, "/Users/x/Unknown.docx"), Some(30));
        // ≥2 panes, no match → none.
        assert_eq!(resolve_pane(&c, "/Users/x/Unknown.docx"), None);
        // Empty → none.
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

        // Answer it.
        route_response(BridgeResponse::ok(
            Some(json!(corr)),
            json!({ "text": "hello world" }),
        ));

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
}
