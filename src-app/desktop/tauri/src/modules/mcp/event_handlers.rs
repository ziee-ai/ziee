//! MCP Server Event Handlers
//!
//! Handles MCP-server-related events for the desktop app.
//!
//! Mirrors `crate::modules::llm_provider::event_handlers::AutoAssignProviderHandler`.
//! On a single-admin desktop the user MCP page is hidden; every install
//! is system-scope. To make those system servers reach the single user
//! without manual group assignment, we auto-assign each new system
//! server to every existing user group on creation. The repository's
//! `assign_to_group` uses `ON CONFLICT (group_id, mcp_server_id) DO
//! NOTHING`, so the handler is idempotent (safe to re-deliver, safe to
//! re-run via the boot backfill below).

use std::sync::Arc;

/// Auto-assigns a newly-created system MCP server to every user group.
/// Triggered by `AppEvent::McpServer(McpServerEvent::SystemServerCreated)`.
pub struct AutoAssignMcpServerHandler;

impl AutoAssignMcpServerHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[ziee::async_trait]
impl ziee::EventHandler for AutoAssignMcpServerHandler {
    async fn handle(
        &self,
        event: &(dyn std::any::Any + Send + Sync),
        _pool: &sqlx::PgPool,
    ) -> std::result::Result<(), ziee::AppError> {
        // The framework `EventHandler` erases the event to `&dyn Any`; recover
        // the app's concrete `AppEvent`.
        let Some(event) = event.downcast_ref::<ziee::AppEvent>() else {
            return Ok(());
        };
        if let ziee::AppEvent::McpServer(ziee::McpServerEvent::SystemServerCreated {
            server_id,
        }) = event
        {
            tracing::info!(
                "Auto-assigning new system MCP server {} to all groups",
                server_id
            );

            match ziee::Repos.group.get_all().await {
                Ok(groups) => {
                    let group_count = groups.len();
                    let mut ok = 0_usize;
                    let mut err = 0_usize;
                    for group in groups {
                        // NB: `Repos.mcp.assign_to_group(group_id, server_id)`
                        // despite the method's misleading parameter names.
                        // The wrapper internally swaps to the free function's
                        // canonical (group_id, server_id) order — every
                        // in-tree caller (handlers/groups.rs:204) uses this
                        // positional convention. Passing them the other way
                        // around silently fails the is_system check
                        // (looks up the group as a server → not-found).
                        match ziee::Repos
                            .mcp
                            .assign_to_group(group.id, *server_id)
                            .await
                        {
                            Ok(()) => ok += 1,
                            Err(e) => {
                                err += 1;
                                tracing::warn!(
                                    server_id = %server_id,
                                    group_id = %group.id,
                                    error = %e,
                                    "auto-assign: assign_to_group failed"
                                );
                            }
                        }
                    }
                    tracing::info!(
                        "System MCP server {} auto-assign: {} groups seen, {} ok, {} err",
                        server_id,
                        group_count,
                        ok,
                        err
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        server_id = %server_id,
                        error = %e,
                        "auto-assign: get_all groups failed"
                    );
                }
            }
        }
        Ok(())
    }

    fn handler_name(&self) -> &'static str {
        "Desktop::AutoAssignMcpServer"
    }
}

/// One-shot backfill: assign every existing system MCP server to every
/// existing user group. Idempotent (ON CONFLICT DO NOTHING). Run once
/// at desktop boot, AFTER the embedded server starts and AFTER
/// built-in MCP servers (memory MCP, etc.) have been registered.
///
/// Why a backfill in addition to the per-event handler:
///   Built-in system servers are registered every boot via an
///   "insert-if-absent" path that does NOT necessarily emit
///   `SystemServerCreated`. Without the backfill, a fresh desktop
///   install would have memory MCP in `mcp_servers` but zero rows in
///   `user_group_mcp_servers`, and the admin user would never see it
///   in the MCP picker. The backfill closes that gap and remains safe
///   on every subsequent boot.
pub async fn backfill_system_mcp_assignments() -> Result<(), ziee::AppError> {
    let groups = ziee::Repos.group.get_all().await?;
    if groups.is_empty() {
        tracing::debug!(
            "backfill_system_mcp_assignments: no groups yet — nothing to do"
        );
        return Ok(());
    }

    // Page through every system server. The repository's list API
    // requires a page+per_page; pick a per-page large enough that a
    // single fetch covers any realistic desktop install (the single
    // admin is unlikely to have thousands of system MCP servers).
    let list = ziee::Repos
        .mcp
        .list_system_servers(1, 10_000, None, None)
        .await?;

    let mut pairs = 0_usize;
    for server in &list.servers {
        for group in &groups {
            // See the comment on the per-event handler above: positional
            // order is (group_id, server_id) despite the wrapper method's
            // misleading parameter names.
            let _ = ziee::Repos.mcp.assign_to_group(group.id, server.id).await;
            pairs += 1;
        }
    }

    tracing::info!(
        "backfill_system_mcp_assignments: ensured {} (server, group) assignments across {} servers / {} groups",
        pairs,
        list.servers.len(),
        groups.len()
    );
    Ok(())
}
