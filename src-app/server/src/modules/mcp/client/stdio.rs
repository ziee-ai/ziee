use async_trait::async_trait;
use rmcp::{ServiceExt, transport::TokioChildProcess, service::RunningService};
use rmcp::model::{CallToolRequestParam, GetPromptRequestParam, ReadResourceRequestParam};
use std::borrow::Cow;
use std::path::PathBuf;
use tokio::process::Command;
use uuid::Uuid;

use super::traits::{
    McpClient, Prompt, PromptArgument, PromptResult, Resource, Tool, ToolContent, ToolResult,
};
use crate::common::AppError;
use crate::modules::code_sandbox;
use crate::modules::code_sandbox::backend::vm_long_lived;
use crate::modules::code_sandbox::mcp_spawn::{
    self, McpSandboxTransport, McpSpawnRequest,
};
use crate::modules::mcp::models::{McpServer, TransportType};
use crate::modules::mcp::utils::embedded;

// Security: Command allowlist (Phase 1)
const ALLOWED_COMMANDS: &[&str] = &["npx", "uvx", "python", "python3", "node", "deno"];

// Security: Environment variable blocklist (Phase 1)
const BLOCKED_ENV_VARS: &[&str] = &[
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SECRET_KEY",
    "DATABASE_PASSWORD",
    "DB_PASSWORD",
    "PGPASSWORD",
    "MYSQL_PASSWORD",
    "REDIS_PASSWORD",
    "API_SECRET",
    "SECRET_KEY",
    "PRIVATE_KEY",
    "JWT_SECRET",
    "ENCRYPTION_KEY",
];

/// Resolve command to embedded binary if applicable
/// Returns (resolved_command, prepended_args)
fn resolve_command(cmd: &str) -> Result<(PathBuf, Vec<String>), AppError> {
    match cmd {
        "uvx" => {
            // Resolve to embedded UV binary: uvx → {uv_path} tool run
            let uv_path = embedded::get_uv_path()?;
            Ok((uv_path.clone(), vec!["tool".to_string(), "run".to_string()]))
        }
        "npx" => {
            // Resolve to embedded Bun binary: npx → {bun_path} x
            let bun_path = embedded::get_bun_path()?;
            Ok((bun_path.clone(), vec!["x".to_string()]))
        }
        "python" | "python3" => {
            // Resolve to embedded UV binary: python → {uv_path} run python
            // UV bundles Python, so this provides a self-contained Python runtime
            let uv_path = embedded::get_uv_path()?;
            Ok((uv_path.clone(), vec!["run".to_string(), cmd.to_string()]))
        }
        "node" => {
            // Resolve to embedded Bun binary: node → {bun_path} run
            // Bun is Node.js compatible and can run JavaScript files
            let bun_path = embedded::get_bun_path()?;
            Ok((bun_path.clone(), vec!["run".to_string()]))
        }
        _ => {
            // Use command as-is (deno, etc. need to be installed separately)
            Ok((PathBuf::from(cmd), vec![]))
        }
    }
}

pub struct StdioMcpClient {
    server_id: Uuid,
    server_config: McpServer,
    service: Option<RunningService<rmcp::RoleClient, ()>>,
    /// Sandboxed VM-backend session, held alive for the duration of the
    /// MCP service. Dropping it sends `KillProcess` to the agent and
    /// releases the per-flavor inflight guard so the VM can be reaped.
    /// `None` for non-sandboxed and Linux-sandboxed paths.
    _vm_session: Option<vm_long_lived::LongLivedSession>,
}

impl StdioMcpClient {
    pub fn new(server: McpServer) -> Result<Self, AppError> {
        if server.transport_type != TransportType::Stdio {
            return Err(AppError::bad_request("INVALID_TRANSPORT", "Only stdio transport supported"));
        }

        Ok(Self {
            server_id: server.id,
            server_config: server,
            service: None,
            _vm_session: None,
        })
    }

    /// `true` if this server is sandbox-eligible AND the sandbox is up.
    /// Only `is_system && stdio && run_in_sandbox` servers route through
    /// the sandbox; user-owned servers ignore the column (the UI hides
    /// the toggle for them anyway).
    fn should_sandbox(&self) -> bool {
        self.server_config.is_system
            && self.server_config.transport_type == TransportType::Stdio
            && self.server_config.run_in_sandbox
            && code_sandbox::config::get_state().is_some()
    }

    /// Non-sandboxed connect: original spawn-on-host path. Preserved
    /// byte-for-byte from prior releases for every non-`run_in_sandbox`
    /// server.
    async fn connect_native(&mut self) -> Result<(), AppError> {
        let command = self.create_command()?;
        let transport = TokioChildProcess::new(command).map_err(|e| {
            tracing::error!(server_id = %self.server_id, error = %e, "Failed to create transport");
            AppError::internal_error(format!("Failed to create transport: {}", e))
        })?;
        let service = ().serve(transport).await.map_err(|e| {
            tracing::error!(server_id = %self.server_id, error = %e, "Failed to connect to MCP server");
            AppError::internal_error(format!("Failed to connect: {}", e))
        })?;
        self.service = Some(service);
        tracing::info!(server_id = %self.server_id, "MCP server connection established");
        Ok(())
    }

    /// Sandboxed connect: builds an `McpSpawnRequest` and routes through
    /// `mcp_spawn::start_mcp_in_sandbox`, which on Linux spawns bwrap
    /// directly and on macOS/Windows tunnels through the per-flavor VM
    /// agent session.
    async fn connect_sandboxed(&mut self) -> Result<(), AppError> {
        let cmd = self.server_config.command.as_ref().ok_or_else(|| {
            AppError::bad_request("MISSING_COMMAND", "Missing command")
        })?;
        if !ALLOWED_COMMANDS.contains(&cmd.as_str()) {
            return Err(AppError::bad_request(
                "INVALID_COMMAND",
                format!("Command '{}' not in allowlist {:?}", cmd, ALLOWED_COMMANDS),
            ));
        }
        let (resolved_command, prepended_args) = resolve_command(cmd)?;
        let server_args = self
            .server_config
            .args
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let extra_setenv = Self::filter_env(&self.server_config.environment_variables);

        let state = code_sandbox::config::get_state().ok_or_else(|| {
            AppError::internal_error("code_sandbox is not initialised — sandboxed MCP cannot start")
        })?;

        let req = McpSpawnRequest {
            server_id: self.server_id,
            original_command: cmd.clone(),
            resolved_command,
            prepended_args,
            server_args,
            extra_setenv,
        };

        let transport = mcp_spawn::start_mcp_in_sandbox(&state, req).await?;
        match transport {
            McpSandboxTransport::LinuxBwrap(child) => {
                let service = ().serve(child).await.map_err(|e| {
                    AppError::internal_error(format!("rmcp serve (sandboxed/linux): {}", e))
                })?;
                self.service = Some(service);
            }
            McpSandboxTransport::VmSession { io, session } => {
                let (rd, wr) = tokio::io::split(io);
                let transport = rmcp::transport::async_rw::AsyncRwTransport::new_client(rd, wr);
                let service = ().serve(transport).await.map_err(|e| {
                    AppError::internal_error(format!("rmcp serve (sandboxed/vm): {}", e))
                })?;
                self.service = Some(service);
                self._vm_session = Some(session);
            }
        }
        tracing::info!(
            server_id = %self.server_id,
            "Sandboxed MCP server connection established"
        );
        Ok(())
    }

    /// Drop every blocked env var. Public-ish (pub(super)) so the test
    /// suite can assert the filter independently of the rest of
    /// `connect()` setup.
    pub(super) fn filter_env(env: &serde_json::Value) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(obj) = env.as_object() {
            for (k, v) in obj {
                if BLOCKED_ENV_VARS.contains(&k.as_str()) {
                    continue;
                }
                if let Some(s) = v.as_str() {
                    out.push((k.clone(), s.to_string()));
                }
            }
        }
        out
    }

    fn create_command(&self) -> Result<Command, AppError> {
        let cmd = self.server_config.command.as_ref()
            .ok_or_else(|| AppError::bad_request("MISSING_COMMAND", "Missing command"))?;

        // Security: Validate command against allowlist
        if !ALLOWED_COMMANDS.contains(&cmd.as_str()) {
            return Err(AppError::bad_request(
                "INVALID_COMMAND",
                format!("Command '{}' is not allowed. Allowed commands: {:?}", cmd, ALLOWED_COMMANDS)
            ));
        }

        // Transparent command resolution: uvx → uv, npx → bun
        let (resolved_cmd, prepended_args) = resolve_command(cmd)?;

        tracing::debug!(
            server_id = %self.server_id,
            original_cmd = %cmd,
            resolved_cmd = ?resolved_cmd,
            prepended_args = ?prepended_args,
            "Resolved MCP server command"
        );

        let mut command = Command::new(&resolved_cmd);

        // Add prepended arguments (e.g., "tool run" for uvx, "x" for npx)
        for arg in prepended_args {
            command.arg(arg);
        }

        // Add original arguments from server config
        if let Some(arr) = self.server_config.args.as_array() {
            for arg in arr {
                if let Some(arg_str) = arg.as_str() {
                    command.arg(arg_str);
                }
            }
        }

        // Add environment variables
        if let Some(obj) = self.server_config.environment_variables.as_object() {
            for (key, value) in obj {
                // Security: Block dangerous environment variables
                if BLOCKED_ENV_VARS.contains(&key.as_str()) {
                    tracing::warn!(
                        server_id = %self.server_id,
                        env_var = %key,
                        "Blocked attempt to set dangerous environment variable"
                    );
                    continue;
                }

                if let Some(val) = value.as_str() {
                    command.env(key, val);
                }
            }
        }

        Ok(command)
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    async fn connect(&mut self) -> Result<(), AppError> {
        if self.is_connected() {
            return Ok(());
        }

        // Audit logging
        tracing::info!(
            server_id = %self.server_id,
            server_name = %self.server_config.name,
            transport = "stdio",
            sandboxed = self.should_sandbox(),
            "MCP server connection initiated"
        );

        if self.should_sandbox() {
            self.connect_sandboxed().await
        } else {
            self.connect_native().await
        }
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        if let Some(service) = self.service.take() {
            tracing::info!(
                server_id = %self.server_id,
                "MCP server disconnection initiated"
            );
            let _ = service.cancel().await;
            tracing::info!(
                server_id = %self.server_id,
                "MCP server disconnected"
            );
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.service.is_some()
    }

    async fn list_tools(&mut self) -> Result<Vec<Tool>, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        let result = service.list_tools(Default::default()).await
            .map_err(|e| AppError::internal_error(format!("Failed to list tools: {}", e)))?;

        Ok(result.tools.into_iter().map(|t| Tool {
            name: t.name.to_string(),
            description: t.description.map(|d| d.to_string()),
            input_schema: serde_json::Value::Object((*t.input_schema).clone()),
        }).collect())
    }

    async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
        _message_id: Option<uuid::Uuid>,
        _sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
        _elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
    ) -> Result<ToolResult, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        let args_map = arguments.as_object().cloned();

        let result = service.call_tool(CallToolRequestParam {
            name: Cow::Owned(name.to_string()),
            arguments: args_map,
        }).await
        .map_err(|e| AppError::internal_error(format!("Tool call failed: {}", e)))?;

        Ok(ToolResult {
            content: result.content.into_iter().map(|c| {
                // Convert rmcp ToolContent to our ToolContent
                ToolContent {
                    content: serde_json::to_value(c).unwrap_or_default(),
                }
            }).collect(),
            is_error: result.is_error.unwrap_or(false),
        })
    }

    async fn list_resources(&mut self) -> Result<Vec<Resource>, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        let result = service.list_resources(Default::default()).await
            .map_err(|e| AppError::internal_error(format!("Failed to list resources: {}", e)))?;

        Ok(result.resources.into_iter().map(|r| Resource {
            uri: r.uri.to_string(),
            name: r.name.to_string(),
            description: r.description.as_ref().map(|d| d.to_string()),
            mime_type: r.mime_type.as_ref().map(|m| m.to_string()),
        }).collect())
    }

    async fn read_resource(&mut self, uri: &str) -> Result<serde_json::Value, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        let result = service.read_resource(ReadResourceRequestParam {
            uri: uri.to_string(),
        }).await
        .map_err(|e| AppError::internal_error(format!("Failed to read resource: {}", e)))?;

        serde_json::to_value(result.contents)
            .map_err(|e| AppError::internal_error(format!("Failed to serialize resource: {}", e)))
    }

    async fn list_prompts(&mut self) -> Result<Vec<Prompt>, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        // rmcp returns an empty list (or errors) for servers that don't
        // advertise the prompts capability. We map any error to an empty
        // Vec so callers don't have to special-case the missing-capability
        // path — matches the behaviour of HttpMcpClient::list_prompts.
        let result = match service.list_prompts(Default::default()).await {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };

        Ok(result.prompts.into_iter().map(|p| Prompt {
            name: p.name.to_string(),
            description: p.description.map(|d| d.to_string()),
            arguments: p.arguments.unwrap_or_default().into_iter().map(|a| PromptArgument {
                name: a.name.to_string(),
                description: a.description.map(|d| d.to_string()),
                required: a.required.unwrap_or(false),
            }).collect(),
        }).collect())
    }

    async fn get_prompt(
        &mut self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<PromptResult, AppError> {
        let service = self.service.as_ref()
            .ok_or_else(|| AppError::internal_error("Not connected"))?;

        let args_map = arguments.and_then(|v| {
            v.as_object().map(|o| o.clone().into_iter().collect())
        });

        let result = service.get_prompt(GetPromptRequestParam {
            name: name.to_string(),
            arguments: args_map,
        }).await
        .map_err(|e| AppError::internal_error(format!("get_prompt failed: {}", e)))?;

        // Convert rmcp's typed PromptMessage list back to opaque JSON values
        // to match the HttpMcpClient shape; callers don't need rmcp types.
        let messages = result.messages.into_iter()
            .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
            .collect();

        Ok(PromptResult {
            description: result.description.map(|d| d.to_string()),
            messages,
        })
    }

    async fn ping(&mut self) -> Result<(), AppError> {
        // rmcp doesn't currently expose `ping` as a high-level method.
        // For stdio transport, liveness is implicit in the child process
        // being alive, so we report success when the service handle exists.
        // If rmcp later exposes ping, swap to that.
        if self.service.is_some() {
            Ok(())
        } else {
            Err(AppError::internal_error("Not connected"))
        }
    }

    async fn cancel(&mut self, _request_id: i64, _reason: &str) -> Result<(), AppError> {
        // The rmcp stdio wrapper doesn't expose a notification-send API for
        // `notifications/cancelled`; dropping the child process is how stdio
        // calls are abandoned. No-op here (best-effort per the trait contract).
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn server_template() -> McpServer {
        McpServer {
            id: Uuid::new_v4(),
            user_id: None,
            name: "test".into(),
            display_name: "Test".into(),
            description: None,
            enabled: true,
            is_system: true,
            is_built_in: false,
            transport_type: TransportType::Stdio,
            command: Some("python3".into()),
            args: serde_json::Value::Array(vec![]),
            environment_variables: serde_json::Value::Object(Default::default()),
            url: None,
            headers: serde_json::Value::Object(Default::default()),
            timeout_seconds: 30,
            supports_sampling: false,
            usage_mode: crate::modules::mcp::models::UsageMode::Auto,
            max_concurrent_sessions: None,
            run_in_sandbox: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// `should_sandbox` requires ALL of: is_system + stdio + flag +
    /// code_sandbox state initialised. The state check is the only
    /// non-server input — in tests `get_state()` returns None unless an
    /// init has run, so we test the static gating branches here and
    /// leave the state-true path for the Tier-2/3 integration suite.
    #[test]
    fn should_sandbox_requires_is_system() {
        let mut s = server_template();
        s.is_system = false;
        let client = StdioMcpClient::new(s).unwrap();
        assert!(!client.should_sandbox());
    }

    #[test]
    fn should_sandbox_requires_stdio_transport() {
        let mut s = server_template();
        s.transport_type = TransportType::Http;
        // StdioMcpClient::new refuses non-stdio anyway, so go through
        // the field directly to exercise the gate.
        let client = StdioMcpClient {
            server_id: s.id,
            server_config: s,
            service: None,
            _vm_session: None,
        };
        assert!(!client.should_sandbox());
    }

    #[test]
    fn should_sandbox_requires_run_in_sandbox_flag() {
        let mut s = server_template();
        s.run_in_sandbox = false;
        let client = StdioMcpClient::new(s).unwrap();
        assert!(!client.should_sandbox());
    }

    /// The state-uninitialised branch is exercised here (in test
    /// configs init_state isn't called) — the flag is true but
    /// get_state() returns None, so should_sandbox stays false.
    #[test]
    fn should_sandbox_false_when_state_uninitialised() {
        let s = server_template();
        let client = StdioMcpClient::new(s).unwrap();
        assert!(!client.should_sandbox(), "expected false when state is None");
    }

    #[test]
    fn filter_env_drops_blocked_keys() {
        let env = json!({
            "FOO": "ok",
            "JWT_SECRET": "leak",
            "DATABASE_PASSWORD": "leak",
            "BAR": "ok",
        });
        let filtered = StdioMcpClient::filter_env(&env);
        let keys: Vec<&str> = filtered.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"FOO"));
        assert!(keys.contains(&"BAR"));
        assert!(!keys.contains(&"JWT_SECRET"));
        assert!(!keys.contains(&"DATABASE_PASSWORD"));
    }

    #[test]
    fn filter_env_handles_non_string_values_gracefully() {
        // Non-string values are dropped silently (the original code
        // also skipped them via `.as_str()`); we preserve that.
        let env = json!({ "OK": "yes", "NUM": 42, "OBJ": {"k": "v"} });
        let filtered = StdioMcpClient::filter_env(&env);
        assert_eq!(filtered, vec![("OK".to_string(), "yes".to_string())]);
    }

    #[test]
    fn filter_env_returns_empty_for_non_object() {
        let env = json!(null);
        assert_eq!(StdioMcpClient::filter_env(&env), Vec::<(String, String)>::new());
    }
}
