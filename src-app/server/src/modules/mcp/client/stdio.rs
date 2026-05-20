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
        })
    }

    fn create_command(&self) -> Result<Command, AppError> {
        let cmd = self.server_config.command.as_ref()
            .ok_or_else(|| AppError::bad_request("MISSING_COMMAND", "Missing command"))?;

        // Security: Validate command against allowlist
        if !ALLOWED_COMMANDS.contains(&cmd.as_str()) {
            return Err(AppError::bad_request(
                "INVALID_COMMAND",
                &format!("Command '{}' is not allowed. Allowed commands: {:?}", cmd, ALLOWED_COMMANDS)
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
            "MCP server connection initiated"
        );

        let command = self.create_command()?;
        let transport = TokioChildProcess::new(command)
            .map_err(|e| {
                tracing::error!(
                    server_id = %self.server_id,
                    error = %e,
                    "Failed to create transport"
                );
                AppError::internal_error(format!("Failed to create transport: {}", e))
            })?;

        let service = ().serve(transport).await
            .map_err(|e| {
                tracing::error!(
                    server_id = %self.server_id,
                    error = %e,
                    "Failed to connect to MCP server"
                );
                AppError::internal_error(format!("Failed to connect: {}", e))
            })?;

        self.service = Some(service);

        tracing::info!(
            server_id = %self.server_id,
            "MCP server connection established"
        );

        Ok(())
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

        let args_map = if let Some(obj) = arguments.as_object() {
            Some(obj.clone())
        } else {
            None
        };

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
}
