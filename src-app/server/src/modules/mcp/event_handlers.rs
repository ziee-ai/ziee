//! Event handlers for the MCP module.
//!
//! Currently a single handler: `McpSessionCleanupHandler` evicts any
//! pooled `McpSession` for a server that just got deleted. Without
//! this, a server deleted via the admin UI or via the hub Re-install
//! path (which deletes + re-inserts with a NEW uuid) would leave its
//! `McpSession` — and the stdio subprocess / HTTP keepalive behind it
//! — orphaned in `McpSessionManager::sessions` until process exit.
//!
//! Defensive today: every current handler call site uses
//! `get_or_create_with_context`, which produces ephemeral sessions
//! that are never stored in the pool, so the pool is effectively
//! empty at runtime. If a future change flips a call site to the
//! pooled `get_or_create` path (e.g. for connection reuse across
//! tool calls), this handler keeps the design invariant — "deleting
//! the row tears down the session" — without further wiring.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::AppError;
use crate::core::events::{AppEvent, EventHandler};
use crate::modules::mcp::client::manager as session_manager;
use crate::modules::mcp::events::McpServerEvent;

pub struct McpSessionCleanupHandler;

impl McpSessionCleanupHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl EventHandler for McpSessionCleanupHandler {
    async fn handle(&self, event: &AppEvent, _pool: &PgPool) -> Result<(), AppError> {
        let server_id = match event {
            AppEvent::McpServer(McpServerEvent::SystemServerDeleted { server_id })
            | AppEvent::McpServer(McpServerEvent::UserServerDeleted { server_id, .. }) => {
                *server_id
            }
            _ => return Ok(()),
        };

        // None in pre-init / test scaffolding — silently noop.
        let Some(manager) = session_manager::global() else {
            tracing::debug!(
                server_id = %server_id,
                "mcp::session_cleanup: skipped — global session manager not installed"
            );
            return Ok(());
        };

        // `close` is no-op when the server_id wasn't pooled. Errors
        // here are NOT bubbled up — the row is already gone and the
        // operator can't act on a session-cleanup failure separately
        // from the delete that just succeeded. Log + carry on.
        if let Err(e) = manager.close(server_id).await {
            tracing::warn!(
                server_id = %server_id,
                error = %e,
                "mcp::session_cleanup: close() failed; session may be leaked until process exit"
            );
        } else {
            tracing::debug!(
                server_id = %server_id,
                "mcp::session_cleanup: pooled session evicted (if any)"
            );
        }

        Ok(())
    }

    fn handler_name(&self) -> &'static str {
        "McpModule::SessionCleanup"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_name_is_stable() {
        // The EventBus logs this string on every dispatch; renaming
        // it silently would break log-based debugging. Lock it in.
        assert_eq!(
            McpSessionCleanupHandler::new().handler_name(),
            "McpModule::SessionCleanup"
        );
    }

    #[tokio::test]
    async fn global_returns_none_when_unset() {
        // Defensive: handler reads `global()` and must tolerate None
        // (unit tests / alternate boot paths). The OnceLock can't be
        // reset between tests in the same binary, so we don't assert
        // None here unconditionally — instead check that the read
        // doesn't panic, regardless of state.
        let _ = session_manager::global();
    }
}
