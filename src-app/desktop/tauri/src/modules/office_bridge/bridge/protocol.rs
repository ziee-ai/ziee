//! JSON-RPC 2.0 envelope types for the bridge protocol (ITEM-5).
//!
//! The task pane and the daemon speak JSON-RPC 2.0 over the `/bridge` WSS
//! socket. This module owns only the wire *shapes* — the request/response/event
//! envelopes. Method dispatch (`list_open_documents`, `read_document`,
//! `edit_document`, …) lands in ITEM-9; for now `/bridge` simply echoes received
//! text frames back (like the proven spike), so these types are the scaffold the
//! dispatcher will deserialize into but are not yet routed.
//!
//! `id`/`params`/`result` are kept as `serde_json::Value` so the envelope stays
//! method-agnostic (JSON-RPC ids may be a number, string, or null; params/result
//! are per-method). All optional fields are `skip_serializing_if` so the emitted
//! JSON stays minimal and spec-clean.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The JSON-RPC version literal every envelope carries.
pub const JSONRPC_VERSION: &str = "2.0";

/// A request/notification from the task pane (or daemon) to the bridge.
///
/// `session_token`, `host`, and `doc_id` are bridge-specific envelope fields
/// carried alongside the standard JSON-RPC trio so a single frame both
/// authenticates and identifies its target document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    /// Always `"2.0"`.
    pub jsonrpc: String,
    /// JSON-RPC request id (number/string); absent for a notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// The invoked method (e.g. `"list_open_documents"`).
    pub method: String,
    /// Method parameters (per-method shape).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Per-session bridge token (DEC-6). Present on frames that must
    /// authenticate; the WSS upgrade already validated the socket, so this is
    /// belt-and-suspenders for frame-level routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    /// The Office host the pane is running in (`"word"`/`"excel"`/`"powerpoint"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The in-pane document identifier the frame targets, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeError {
    /// Numeric JSON-RPC error code.
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured error data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A response to a [`BridgeRequest`]. Exactly one of `result` / `error` is set
/// (JSON-RPC 2.0 rule); the envelope does not enforce that at the type level to
/// stay a faithful wire mirror.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    /// Always `"2.0"` on the wire. `default` on deserialize so a peer reply that
    /// omits `jsonrpc` still parses + routes (rather than being silently dropped and
    /// timing out); the value itself is not validated.
    #[serde(default)]
    pub jsonrpc: String,
    /// Echoes the request id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// The successful result payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The error, when the call failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeError>,
}

/// A server→pane push (selection change, open/close, …). A JSON-RPC
/// notification whose `method` is fixed to `"event"`, with the concrete event
/// carried in `params`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEvent {
    /// Always `"2.0"`.
    pub jsonrpc: String,
    /// Always `"event"`.
    pub method: String,
    /// The event payload.
    pub params: Value,
}

impl BridgeResponse {
    /// Build a success response echoing `id`.
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response echoing `id`.
    pub fn err(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(BridgeError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

impl BridgeEvent {
    /// Build an `"event"` notification with the given payload.
    pub fn new(params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "event".to_string(),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_roundtrips_with_bridge_fields() {
        let raw = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "list_open_documents",
            "params": {"filter": "word"},
            "session_token": "abc",
            "host": "word",
            "doc_id": "Report.docx"
        });
        let req: BridgeRequest = serde_json::from_value(raw).unwrap();
        assert_eq!(req.method, "list_open_documents");
        assert_eq!(req.host.as_deref(), Some("word"));
        assert_eq!(req.session_token.as_deref(), Some("abc"));
        assert_eq!(req.id, Some(json!(7)));
    }

    #[test]
    fn notification_without_id_or_optional_fields() {
        let raw = json!({"jsonrpc": "2.0", "method": "ping"});
        let req: BridgeRequest = serde_json::from_value(raw).unwrap();
        assert!(req.id.is_none());
        assert!(req.params.is_none());
        assert!(req.host.is_none());
        // Re-serializing omits the None fields entirely.
        let back = serde_json::to_value(&req).unwrap();
        assert_eq!(back, json!({"jsonrpc": "2.0", "method": "ping"}));
    }

    #[test]
    fn response_ok_and_err_are_exclusive() {
        let ok = BridgeResponse::ok(Some(json!(1)), json!({"docs": []}));
        let v = serde_json::to_value(&ok).unwrap();
        assert!(v.get("result").is_some());
        assert!(v.get("error").is_none());

        let err = BridgeResponse::err(Some(json!(1)), -32601, "method not found");
        let v = serde_json::to_value(&err).unwrap();
        assert!(v.get("result").is_none());
        assert_eq!(v["error"]["code"], json!(-32601));
    }

    #[test]
    fn response_deserializes_without_jsonrpc_field() {
        // A peer reply that omits `jsonrpc` must still parse (→ route) rather than
        // fail deserialization and be silently dropped (which would hang the caller
        // to the timeout). See the `#[serde(default)]` on `BridgeResponse::jsonrpc`.
        let raw = json!({ "id": 7, "result": { "text": "ok" } });
        let resp: BridgeResponse = serde_json::from_value(raw).expect("parses without jsonrpc");
        assert_eq!(resp.id, Some(json!(7)));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn event_method_is_fixed() {
        let ev = BridgeEvent::new(json!({"kind": "selection_changed"}));
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["method"], "event");
        assert_eq!(v["jsonrpc"], "2.0");
    }
}
