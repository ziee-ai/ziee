use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::config::CodeSandboxConfig;
use crate::modules::code_sandbox::models::ConversationFile;

/// JSON-RPC 2.0 request envelope. The sandbox handler accepts only a
/// minimal subset: `initialize`, `tools/list`, `tools/call`.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default = "default_jsonrpc")]
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
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL: i32 = -32603;

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
}

/// Per-call context built by the HTTP handler before dispatching a tool.
#[derive(Debug, Clone)]
pub struct SandboxContext {
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub workspace: PathBuf,
    pub files: Arc<Vec<ConversationFile>>,
}

/// Cached, process-lifetime hardening capabilities. Populated once at
/// `code_sandbox::init()`; every per-call code path reads from here so
/// we never re-probe the environment per request.
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
    pub caps: HardeningCapabilities,
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
