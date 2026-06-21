use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;

use super::session::McpSession;
use crate::common::AppError;
use crate::core::{config::Config, Repos};
use crate::modules::auth::jwt::Claims;
use crate::modules::mcp::tool_calls::models::{McpCallContext, McpToolCallSource};

/// Process-wide handle to the session manager constructed in
/// `main.rs`. The event-handler path (`McpSessionCleanupHandler`)
/// needs to call `close(server_id)` when a server row is deleted —
/// but event handlers are registered via the `AppModule` trait which
/// runs BEFORE `main.rs` instantiates the session manager. The
/// Axum-Extension injection used by HTTP handlers can't reach them.
///
/// `main.rs` calls `set_global(...)` once at boot. Read via
/// `global()`; returns `None` in pre-init test scaffolding (unit
/// tests that don't go through `main.rs`).
static MCP_SESSION_MANAGER: OnceLock<Arc<McpSessionManager>> = OnceLock::new();

/// Install the process-wide session-manager handle. Idempotent on the
/// second call (subsequent `set` attempts are silently dropped — boot
/// only calls this once, but unit-test harnesses might call it from a
/// shared setup function).
pub fn set_global(manager: Arc<McpSessionManager>) {
    let _ = MCP_SESSION_MANAGER.set(manager);
}

/// Read the process-wide session-manager handle. None when called
/// before `set_global` (e.g. unit tests that don't boot `main.rs`).
pub fn global() -> Option<Arc<McpSessionManager>> {
    MCP_SESSION_MANAGER.get().cloned()
}

pub struct McpSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<McpSession>>>>>,
    config: Arc<Config>,
}

impl McpSessionManager {
    #[allow(dead_code)] // Used in main.rs (binary), not in library
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn get_or_create(
        &self,
        server_id: Uuid,
    ) -> Result<Arc<RwLock<McpSession>>, AppError> {
        // Check if session exists
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&server_id) {
                return Ok(session.clone());
            }
        }

        // Load server config from database
        let server = Repos.mcp.get_any_server(server_id).await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        // Check if server is enabled
        if !server.enabled {
            return Err(AppError::bad_request("server_disabled", "Server is disabled"));
        }

        // Create new session
        let session = McpSession::new(server).await?;
        let session = Arc::new(RwLock::new(session));

        // Store session
        let mut sessions = self.sessions.write().await;
        sessions.insert(server_id, session.clone());

        Ok(session)
    }

    /// Get or create a session with conversation context headers injected.
    /// Always creates an EPHEMERAL (non-pooled) session — for both built-in
    /// servers (with X-Conversation-Id / X-Message-Id / a short-lived JWT) and
    /// regular servers (so parallel tool execution doesn't share one session).
    /// The ephemerality is what makes stamping `call_ctx` race-free: every
    /// tool call gets its own freshly-stamped session. `source` records how
    /// the call was triggered (chat / rest / always / approval / sampling).
    pub async fn get_or_create_with_context(
        &self,
        server_id: Uuid,
        user_id: Uuid,
        conversation_id: Option<Uuid>,
        branch_id: Option<Uuid>,
        message_id: Option<Uuid>,
        tool_use_id: Option<String>,
        source: McpToolCallSource,
    ) -> Result<Arc<RwLock<McpSession>>, AppError> {
        let server = Repos.mcp.get_any_server(server_id).await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        if !server.enabled {
            return Err(AppError::bad_request("server_disabled", "Server is disabled"));
        }

        // Recording context stamped onto whichever session we build below.
        let call_ctx = McpCallContext {
            user_id: Some(user_id),
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            source,
            server_name: server.name.clone(),
            is_built_in: server.is_built_in,
            // Stamped post-creation by the workflow dispatcher (set_workflow_run);
            // every other caller leaves it None.
            workflow_run_id: None,
        };

        // For built-in servers: create ephemeral session with dynamic headers
        if server.is_built_in {
            let mut server_with_ctx = server.clone();

            let mut headers = server.headers
                .as_object()
                .cloned()
                .unwrap_or_default();

            if let Some(cid) = conversation_id {
                headers.insert(
                    "x-conversation-id".to_string(),
                    Value::String(cid.to_string()),
                );
            }
            if let Some(msg_id) = message_id {
                headers.insert(
                    "x-message-id".to_string(),
                    Value::String(msg_id.to_string()),
                );
            }

            // Inject Authorization header with a short-lived JWT if not already set
            if !headers.contains_key("authorization") && !headers.contains_key("Authorization") {
                let token = Self::generate_short_lived_jwt(user_id, &self.config.jwt.secret, 5)?;
                headers.insert(
                    "Authorization".to_string(),
                    Value::String(format!("Bearer {}", token)),
                );
            }

            server_with_ctx.headers = Value::Object(headers);

            // Ephemeral session — not stored in the pool
            let mut session = McpSession::new(server_with_ctx).await?;
            session.set_call_context(call_ctx);
            return Ok(Arc::new(RwLock::new(session)));
        }

        // Non-built-in: create ephemeral session per call (no pool, allows parallel tool execution)
        let mut session = McpSession::new(server).await?;
        session.set_call_context(call_ctx);
        Ok(Arc::new(RwLock::new(session)))
    }

    /// The deployment JWT secret. Used by the workflow `ToolDispatcher` (E9) so
    /// it can pass a secret to `resource_link::persist_links` — letting a tool's
    /// token-based `http://` loopback resource_links be fetched + persisted, not
    /// just in-process `ziee://` host-path links.
    pub fn jwt_secret(&self) -> &str {
        &self.config.jwt.secret
    }

    /// Generate a short-lived JWT for internal service-to-service calls.
    pub fn generate_short_lived_jwt(
        user_id: Uuid,
        secret: &str,
        ttl_seconds: i64,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(ttl_seconds);
        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: "ziee".to_string(),
            aud: "ziee-api".to_string(),
            username: String::new(),
            email: String::new(),
            is_admin: false,
            jti: None,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate internal JWT: {}", e)))
    }

    pub async fn close(&self, server_id: Uuid) -> Result<(), AppError> {
        let session = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&server_id)
        };

        if let Some(session) = session {
            let mut session = session.write().await;
            session.disconnect().await?;
        }

        Ok(())
    }

    #[allow(dead_code)] // Used in main.rs for graceful shutdown (binary only)
    pub async fn close_all(&self) -> Result<(), AppError> {
        let sessions = {
            let mut sessions = self.sessions.write().await;
            
            sessions.drain().collect::<Vec<_>>()
        };

        for (_, session) in sessions {
            let mut session = session.write().await;
            let _ = session.disconnect().await;
        }

        Ok(())
    }

    /// Whether a session for `server_id` is currently pooled. Drives
    /// the cleanup test that asserts `McpSessionCleanupHandler` actually
    /// removed an entry from the pool after a delete event.
    pub async fn contains(&self, server_id: Uuid) -> bool {
        self.sessions.read().await.contains_key(&server_id)
    }

    /// Forcibly insert a placeholder session, bypassing the
    /// `get_or_create` path. Test-only — lets the cleanup test seed
    /// the pool without standing up a real subprocess / HTTP server.
    #[cfg(test)]
    pub async fn insert_for_test(
        &self,
        server_id: Uuid,
        session: Arc<RwLock<McpSession>>,
    ) {
        self.sessions.write().await.insert(server_id, session);
    }

    #[allow(dead_code)] // Phase 3 feature: background task to cleanup idle sessions
    pub async fn cleanup_idle(&self, max_idle_seconds: u64) -> Result<usize, AppError> {
        let to_remove = {
            let sessions = self.sessions.read().await;
            let mut to_remove = Vec::new();

            for (server_id, session) in sessions.iter() {
                let session = session.read().await;
                if session.idle_time().as_secs() > max_idle_seconds {
                    to_remove.push(*server_id);
                }
            }

            to_remove
        };

        for server_id in &to_remove {
            self.close(*server_id).await?;
        }

        Ok(to_remove.len())
    }
}
