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
