use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use sqlx::PgPool;

use super::session::McpSession;
use crate::common::AppError;
use crate::modules::mcp::repository::McpRepository;

pub struct McpSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<McpSession>>>>>,
    pool: PgPool,
}

impl McpSessionManager {
    #[allow(dead_code)] // Used in main.rs (binary), not in library
    pub fn new(pool: PgPool) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pool,
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
        let repo = McpRepository::new(self.pool.clone());
        let server = repo.get_system_server(server_id).await?
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
            let all = sessions.drain().collect::<Vec<_>>();
            all
        };

        for (_, session) in sessions {
            let mut session = session.write().await;
            let _ = session.disconnect().await;
        }

        Ok(())
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
