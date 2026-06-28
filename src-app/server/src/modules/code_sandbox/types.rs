use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::models::ConversationFile;

/// Per-conversation flavor lock. Populated by the FIRST
/// `execute_command` call in a conversation; subsequent calls in the
/// same conversation default to that flavor unless the LLM
/// explicitly requests a different one. A switch within a
/// conversation is logged (so it's auditable) but allowed — it just
/// triggers a fresh mount for the new flavor (both stay live).
pub static CONVERSATION_FLAVOR: Lazy<DashMap<Uuid, String>> = Lazy::new(DashMap::new);

/// Flavor metadata surfaced via the `list_sandbox_environments` MCP
/// tool (Phase 5). The list lives in code rather than the rootfs
/// because the binary needs to advertise flavors BEFORE any rootfs
/// is mounted; future iteration may move human-readable descriptions
/// into a `<rootfs>/.ziee-sandbox-rootfs-flavor.json` sentinel.
pub struct FlavorMetadata {
    pub flavor: &'static str,
    pub description: &'static str,
    pub approximate_size_mb: u64,
}

pub const KNOWN_FLAVORS: &[FlavorMetadata] = &[
    FlavorMetadata {
        flavor: "minimal",
        description: "Shell + coreutils + curl + jq + git + python3 (interpreter only).",
        approximate_size_mb: 57,
    },
    FlavorMetadata {
        flavor: "full",
        description: "minimal + numpy + pandas + torch + R 4.4 + tidyverse + Node 24 (npx) + uv (uvx) + ts-node.",
        approximate_size_mb: 853,
    },
];

/// JSON-RPC 2.0 request envelope. The sandbox handler accepts only a
/// minimal subset: `initialize`, `tools/list`, `tools/call`.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc")]
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

fn default_jsonrpc() -> String {
    "2.0".to_string()
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// JSON-RPC 2.0 standard codes (https://www.jsonrpc.org/specification#error_object).
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL: i32 = -32603;

    /// Invalid JSON was received (the payload was not parseable). Per the
    /// JSON-RPC spec this is `-32700`; the HTTP layer pairs it with 400.
    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self {
            code: Self::PARSE_ERROR,
            message: format!("Parse error: {}", detail.into()),
            data: None,
        }
    }

    /// The JSON was valid but not a valid JSON-RPC request object (`-32600`).
    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self {
            code: Self::INVALID_REQUEST,
            message: format!("Invalid request: {}", detail.into()),
            data: None,
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: Self::METHOD_NOT_FOUND,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self {
            code: Self::INVALID_PARAMS,
            message: format!("Invalid params: {}", detail.into()),
            data: None,
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            code: Self::INTERNAL,
            message: detail.into(),
            data: None,
        }
    }

    /// Map an `AppError` onto the right JSON-RPC error class for the built-in
    /// MCP servers (files / memory / skill / workflow), so client-class errors
    /// surface as method-not-found / invalid-params rather than a generic
    /// internal error. Shared so the built-in handlers can't drift.
    pub fn from_app_error(e: &crate::common::AppError) -> Self {
        match e.status_code() {
            400 if e.error_code() == "UNKNOWN_TOOL" => {
                Self::method_not_found(&e.to_string())
            }
            // 4xx are client-class (bad input / access-denied / not-found /
            // stale) — surface as invalid_params so the LLM sees a client
            // error, not a server crash. skill_mcp / workflow_mcp return 403
            // (hidden / inaccessible / not-owner) and 410 (stale elicit),
            // which the older 400|404-only arm misclassified as internal.
            400 | 403 | 404 | 409 | 410 | 422 => Self::invalid_params(e.to_string()),
            _ => Self::internal(e.to_string()),
        }
    }
}

/// Per-call context built by the HTTP handler before dispatching a tool.
#[derive(Debug, Clone)]
pub struct SandboxContext {
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub workspace: PathBuf,
    pub files: Arc<Vec<ConversationFile>>,
}

/// Host-level capabilities — known at server boot. These don't depend
/// on the sandbox rootfs being mounted, so they're probed unconditionally
/// in `code_sandbox::init()` and stored in `CodeSandboxState.host_caps`.
///
/// `bwrap_path` is required (not `Option`) — if bwrap is missing,
/// `init()` skips the MCP row entirely; this struct is never constructed.
#[derive(Debug, Clone)]
pub struct HostCapabilities {
    pub bwrap_path: PathBuf,
    pub cgroup: CgroupMode,
    pub seccomp: SeccompMode,
}

/// Full hardening capabilities — only complete after the rootfs is
/// lazily mounted on first `execute_command`. Built by merging
/// `HostCapabilities` with the rootfs-dependent `pid_namespace` probe.
/// Cached for the rest of the server's lifetime in `runtime_mount::READY`.
#[derive(Debug, Clone)]
pub struct HardeningCapabilities {
    pub bwrap_path: PathBuf,
    pub pid_namespace: PidNsMode,
    pub cgroup: CgroupMode,
    pub seccomp: SeccompMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidNsMode {
    /// `--unshare-pid --proc /proc` works (bare-metal Linux).
    Strict,
    /// PID-ns probe failed; fall back to `--dev-bind /proc /proc`
    /// (host PIDs visible inside sandbox — info leak, no escape).
    DevBindFallback,
    /// Neither mode works. Sandbox is forced off.
    Disabled,
}

#[derive(Debug, Clone)]
pub enum CgroupMode {
    /// Delegated parent cgroup is writable; per-call scope will be
    /// created at `<parent>/sandbox-<conv_id>-<nanos>/`.
    Delegated(PathBuf),
    /// No cgroup write access. rlimits-only mode.
    None,
}

#[derive(Debug, Clone)]
pub enum SeccompMode {
    /// Filter compiled once at boot; per-call we pipe these bytes to
    /// bwrap's `--seccomp <fd>`.
    Loaded(Arc<Vec<u8>>),
    /// `code_sandbox_seccomp` feature not compiled in.
    NotLinked,
    /// Feature on, but libseccomp failed at runtime. Logged at boot.
    Disabled,
}

/// Global per-process sandbox state, populated at `init()`.
#[derive(Debug)]
pub struct CodeSandboxState {
    pub config: CodeSandboxConfig,
    /// Loopback URL the registered MCP server row points at.
    /// Cached so we don't recompute on every call.
    pub loopback_url: String,
    /// Workspace root: `<data_dir>/sandboxes/`. Per-conversation
    /// subdirs are created on demand under here.
    pub workspace_root: PathBuf,
    /// Cheap, rootfs-independent capabilities probed at boot. The
    /// rootfs-dependent `pid_namespace` field of `HardeningCapabilities`
    /// is populated lazily on the first `execute_command` call and
    /// cached in `runtime_mount::READY`; per-call code paths fetch
    /// the full caps via `runtime_mount::ensure_rootfs_ready(state).await?`.
    pub host_caps: HostCapabilities,
    /// Connection pool for the version manager + repository. Needed
    /// here (rather than threaded through every call) because the
    /// lazy auto-fetch path in `runtime_mount::ensure_rootfs_ready`
    /// must resolve the pinned rootfs version against the DB on the
    /// first `execute_command` of every flavor.
    ///
    /// `Option` so the in-process unit tests that exercise
    /// argv-builder primitives (sandbox.rs, handlers.rs) can construct
    /// a state without a Tokio runtime around the pool; the lazy
    /// fetch path checks `is_some()` and errors with a clear
    /// `code_sandbox not initialized` message if the pool isn't
    /// wired (which never happens in production — `mod.rs::init`
    /// always sets it).
    pub pool: Option<Arc<sqlx::PgPool>>,
}

// =====================================================================
// ConversationIdHeader — axum extractor for the `x-conversation-id`
// request header. Equivalent in behavior to the inline parser in
// `handlers::extract_conversation_id`; we expose it as a proper
// extractor for any future handler that wants typed-extraction
// ergonomics. The existing handlers stay on the inline parser so the
// per-conversation mutex acquisition order doesn't reshuffle.
// =====================================================================

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;

/// Extractor for the optional `x-conversation-id` request header.
///
/// Wraps `Option<Uuid>` rather than `Uuid` because the MCP manager
/// only sets the header when a conversation context exists
/// (`client/manager.rs:88-93`). Requests without a conversation
/// context — `initialize` during MCP discovery, `tools/list` stateless
/// catalog queries — MUST succeed. Requests that actually need the
/// context (`tools/call`) validate the inner Option themselves and
/// reject with a JSON-RPC error if it's None.
///
/// A malformed (non-UUID) header is still rejected at extractor time
/// with 400 — that's a real client bug, not a missing-context case.
#[derive(Debug, Clone, Copy)]
pub struct ConversationIdHeader(pub Option<Uuid>);

impl<S: Send + Sync> FromRequestParts<S> for ConversationIdHeader {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw = match parts.headers.get("x-conversation-id") {
            Some(v) => v,
            None => return Ok(ConversationIdHeader(None)),
        };
        let s = raw
            .to_str()
            .map_err(|_| (StatusCode::BAD_REQUEST, "x-conversation-id is not ASCII"))?;
        let uuid = Uuid::parse_str(s.trim())
            .map_err(|_| (StatusCode::BAD_REQUEST, "x-conversation-id is not a uuid"))?;
        Ok(ConversationIdHeader(Some(uuid)))
    }
}

// =====================================================================
// Tier 1 unit tests — JSON-RPC envelope
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonrpc_request_round_trip() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).expect("parse");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, Some(serde_json::json!(1)));
    }

    #[test]
    fn jsonrpc_request_accepts_missing_jsonrpc_field() {
        let raw = r#"{"id":1,"method":"initialize"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).expect("parse");
        assert_eq!(req.jsonrpc, "2.0"); // default applied
    }

    #[test]
    fn jsonrpc_request_accepts_string_id() {
        let raw = r#"{"jsonrpc":"2.0","id":"abc","method":"x"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).expect("parse");
        assert_eq!(req.id, Some(serde_json::json!("abc")));
    }

    #[test]
    fn jsonrpc_error_helpers_have_canonical_codes() {
        let mnf = JsonRpcError::method_not_found("foo");
        assert_eq!(mnf.code, JsonRpcError::METHOD_NOT_FOUND);
        assert_eq!(mnf.code, -32601);

        let ip = JsonRpcError::invalid_params("bad");
        assert_eq!(ip.code, JsonRpcError::INVALID_PARAMS);
        assert_eq!(ip.code, -32602);

        let internal = JsonRpcError::internal("boom");
        assert_eq!(internal.code, JsonRpcError::INTERNAL);
        assert_eq!(internal.code, -32603);
    }

    // ─── ConversationIdHeader extractor ──────────────────────────

    fn make_parts(headers: Vec<(&str, &str)>) -> axum::http::request::Parts {
        let mut builder = axum::http::Request::builder().uri("/");
        for (k, v) in headers {
            builder = builder.header(k, v);
        }
        let (parts, _) = builder.body(()).unwrap().into_parts();
        parts
    }

    #[tokio::test]
    async fn conversation_id_header_parses_uuid() {
        let mut parts = make_parts(vec![(
            "x-conversation-id",
            "11111111-2222-3333-4444-555555555555",
        )]);
        let ConversationIdHeader(opt) =
            ConversationIdHeader::from_request_parts(&mut parts, &()).await.unwrap();
        assert_eq!(
            opt.unwrap().to_string(),
            "11111111-2222-3333-4444-555555555555"
        );
    }

    #[tokio::test]
    async fn conversation_id_header_missing_returns_none() {
        // initialize + tools/list calls land WITHOUT a conversation
        // context. The extractor must succeed; per-call methods that
        // need the context will reject downstream.
        let mut parts = make_parts(vec![]);
        let ConversationIdHeader(opt) =
            ConversationIdHeader::from_request_parts(&mut parts, &()).await.unwrap();
        assert!(opt.is_none());
    }

    #[tokio::test]
    async fn conversation_id_header_rejects_garbage() {
        // A malformed header IS a client bug — reject at extractor time.
        let mut parts = make_parts(vec![("x-conversation-id", "not-a-uuid")]);
        let err = ConversationIdHeader::from_request_parts(&mut parts, &())
            .await
            .expect_err("garbage must reject");
        assert_eq!(err.0, axum::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn jsonrpc_response_serializes_with_either_result_or_error() {
        let ok = JsonRpcResponse {
            jsonrpc: "2.0",
            id: Some(serde_json::json!(7)),
            result: Some(serde_json::json!({"x": 1})),
            error: None,
        };
        let s = serde_json::to_string(&ok).unwrap();
        assert!(s.contains("\"result\""));
        assert!(!s.contains("\"error\""));

        let err = JsonRpcResponse {
            jsonrpc: "2.0",
            id: None,
            result: None,
            error: Some(JsonRpcError::method_not_found("nope")),
        };
        let s = serde_json::to_string(&err).unwrap();
        assert!(s.contains("\"error\""));
        assert!(!s.contains("\"result\""));
    }
}
