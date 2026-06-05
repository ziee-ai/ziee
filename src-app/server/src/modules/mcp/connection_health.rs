//! Connection-health enforcement for MCP servers.
//!
//! Three entry points share a single underlying probe (the same
//! `run_connection_test` the explicit "Test Connection" UI button
//! uses):
//!
//! 1. Update / enable flow — refuse to flip `enabled: false → true`
//!    when the new config can't connect (handler returns 400 with the
//!    failure detail; other fields in the same PUT still persist).
//! 2. Create flow — if the new server was requested with
//!    `enabled: true` and the probe fails, downgrade to
//!    `enabled: false` and return a warning so the row is preserved
//!    for the user to edit + retry.
//! 3. Boot — every enabled non-built-in MCP server is probed on
//!    server startup; failures flip to `enabled: false` automatically
//!    so users don't see broken servers in their tool lists.
//!
//! Built-in servers (filesystem, memory, code_sandbox, memory_mcp)
//! are SKIPPED — they're owned by the platform, not by user config,
//! and their reachability is the platform's responsibility.

use crate::common::AppError;
use crate::core::Repos;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::client::auth::OAuthClientConfig;
use super::handlers::test_connection::run_connection_test;
use super::models::{McpServer, TransportType};

/// Structured probe failure carrying the underlying reason so the
/// caller can surface it (in the API response, in the boot log, or
/// in the UI toast).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ProbeFailure {
    /// Human-readable reason — taken verbatim from
    /// `TestMcpConnectionResponse.message` (timeout / 401 / bad
    /// command / etc.).
    pub reason: String,
}

/// Wraps a created/updated `McpServer` with an optional connection
/// warning, used by the create handlers when the probe failed and
/// the server was auto-downgraded to `enabled: false`. `None` on
/// success (probe passed, or `enabled: false` was requested so no
/// probe ran).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct McpServerWithHealthWarning {
    pub server: super::models::McpServer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_warning: Option<ProbeFailure>,
}

/// Create-flow enforcement. Call AFTER `Repos.mcp.create_*_server`
/// returns the persisted row. Probes when the new server is
/// `enabled: true` and not built-in; on probe failure, flips
/// `enabled: false` in the DB and returns the updated server with
/// `connection_warning` set. Built-in servers are never probed.
pub async fn enforce_on_create(
    pool: &PgPool,
    server: super::models::McpServer,
    event_bus: &crate::core::events::EventBus,
) -> Result<McpServerWithHealthWarning, AppError> {
    if !server.enabled || server.is_built_in {
        return Ok(McpServerWithHealthWarning {
            server,
            connection_warning: None,
        });
    }

    match probe(pool, &server).await {
        Ok(()) => Ok(McpServerWithHealthWarning {
            server,
            connection_warning: None,
        }),
        Err(failure) => {
            tracing::warn!(
                server_id = %server.id,
                reason = %failure.reason,
                "mcp::health: create-time probe failed; downgrading new server to disabled",
            );
            disable_for_health_failure(pool, server.id).await?;
            event_bus.emit_async(
                super::events::McpServerEvent::auto_disabled(
                    server.id,
                    failure.reason.clone(),
                ),
            );
            // Re-fetch so the response carries the canonical state
            // (enabled=false, updated_at bumped). Falling back to
            // the in-memory copy with `enabled` patched would skip
            // the updated_at refresh.
            let refetched = Repos
                .mcp
                .get_any_server(server.id)
                .await?
                .ok_or_else(|| AppError::internal_error("Server vanished after auto-disable"))?;
            Ok(McpServerWithHealthWarning {
                server: refetched,
                connection_warning: Some(failure),
            })
        }
    }
}

/// Update-flow enforcement. Call AFTER persisting all other fields
/// but BEFORE returning the response. When the update is an
/// enabled-transition (`old_enabled == false && new_enabled == true`)
/// the persisted state is probed; on failure the row's `enabled` is
/// forced back to false in the DB and the function returns a 400
/// `AppError` so the handler short-circuits. Other fields the
/// admin updated in the same PUT stay persisted (the user
/// explicitly chose this trade-off — partial save with a clear
/// error rather than losing every concurrent edit).
pub async fn enforce_on_update_transition(
    pool: &PgPool,
    persisted: super::models::McpServer,
    old_enabled: bool,
    event_bus: &crate::core::events::EventBus,
) -> Result<super::models::McpServer, AppError> {
    let transitioned_to_enabled = persisted.enabled && !old_enabled;
    if !transitioned_to_enabled || persisted.is_built_in {
        return Ok(persisted);
    }

    match probe(pool, &persisted).await {
        Ok(()) => Ok(persisted),
        Err(failure) => {
            tracing::warn!(
                server_id = %persisted.id,
                reason = %failure.reason,
                "mcp::health: update-enable-transition probe failed; reverting to enabled=false",
            );
            disable_for_health_failure(pool, persisted.id).await?;
            event_bus.emit_async(
                super::events::McpServerEvent::auto_disabled(
                    persisted.id,
                    failure.reason.clone(),
                ),
            );
            Err(AppError::bad_request(
                "MCP_ENABLE_FAILED_HEALTH_CHECK",
                format!(
                    "Other changes were saved, but the server could not \
                     be enabled because the connection probe failed: {}",
                    failure.reason
                ),
            ))
        }
    }
}

/// Probe an MCP server's connection. Returns `Ok(())` on a successful
/// `initialize` handshake; `Err(ProbeFailure)` otherwise.
///
/// Loads any stored OAuth config for HTTP servers — same resolution
/// path the explicit Test Connection button uses, so the probe sees
/// the same auth state the runtime would.
pub async fn probe(pool: &PgPool, server: &McpServer) -> Result<(), ProbeFailure> {
    // Resolve stored OAuth config for HTTP servers. Failure to read
    // the config is itself a probe failure — the runtime would fail
    // too, so the caller should see the same outcome.
    let oauth = match server.transport_type {
        TransportType::Http => match Repos.mcp.get_oauth_config(server.id).await {
            Ok(Some(cfg)) => Some(cfg.into_client_config()),
            Ok(None) => None,
            Err(e) => {
                return Err(ProbeFailure {
                    reason: format!("Failed to read OAuth config: {e}"),
                });
            }
        },
        _ => None,
    };
    // `pool` arg is reserved for future probe variants that need DB
    // access beyond the OAuth lookup (e.g. resolving runtime overrides);
    // the OAuth resolution above goes through `Repos.mcp` which holds
    // its own pool reference.
    let _ = pool;

    let response = run_connection_test(server.clone(), oauth).await;
    if response.success {
        Ok(())
    } else {
        Err(ProbeFailure {
            reason: response.message,
        })
    }
}

/// Boot-time health check. Iterates every `enabled = true` MCP server
/// that's not built-in, probes it, and flips `enabled = false` on
/// any failure. Logs each transition.
///
/// Runs as a fire-and-forget background task spawned from `mcp::init`
/// — should NOT block boot. Built-in servers are owned by their
/// respective modules (filesystem, memory_mcp, code_sandbox) and
/// don't go through this path.
///
/// No event emission here: the `EventBus` is built AFTER module
/// init, so it's not in scope at this stage. The on-save handlers
/// (which DO have access via Axum Extension) emit the AutoDisabled
/// event when they downgrade a server. UI pages re-fetch on mount,
/// so a boot-time auto-disable is visible the next time the user
/// opens the MCP servers list — no event channel needed for the
/// boot path specifically.
pub async fn run_startup_health_check(pool: PgPool) {
    let servers = match Repos.mcp.list_enabled_for_health_check().await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = ?e, "mcp::health: failed to list enabled servers for startup check");
            return;
        }
    };

    if servers.is_empty() {
        tracing::debug!("mcp::health: no enabled servers to probe");
        return;
    }

    tracing::info!(
        count = servers.len(),
        "mcp::health: probing enabled MCP servers at startup",
    );

    for server in servers {
        let server_id = server.id;
        let server_name = server.name.clone();
        match probe(&pool, &server).await {
            Ok(()) => {
                tracing::debug!(
                    server_id = %server_id,
                    server_name = %server_name,
                    "mcp::health: server reachable",
                );
            }
            Err(failure) => {
                tracing::warn!(
                    server_id = %server_id,
                    server_name = %server_name,
                    reason = %failure.reason,
                    "mcp::health: auto-disabling unreachable MCP server",
                );
                // Best-effort flip; if the UPDATE itself fails, log
                // and keep going — next boot will retry.
                if let Err(e) = disable_for_health_failure(&pool, server_id).await {
                    tracing::error!(
                        server_id = %server_id,
                        error = ?e,
                        "mcp::health: failed to auto-disable server",
                    );
                }
            }
        }
    }
}

/// UPDATE one row's `enabled` to false. Direct SQL — the public
/// `update_*_mcp_server` paths require the full request shape and
/// run additional validation that's unnecessary for this internal
/// auto-disable.
async fn disable_for_health_failure(
    pool: &PgPool,
    server_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE mcp_servers SET enabled = false, updated_at = NOW() WHERE id = $1",
        server_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}
