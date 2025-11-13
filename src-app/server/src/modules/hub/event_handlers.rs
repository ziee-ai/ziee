// Event handlers for hub module
// Handles cleanup of hub entity tracking when entities are deleted

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::AppError;
use crate::core::events::{AppEvent, EventHandler};
use crate::modules::assistant::events::AssistantEvent;
use crate::modules::mcp::events::McpServerEvent;

/// Cleans up hub_entities records when tracked entities are deleted
pub struct CleanupHubEntitiesHandler;

impl CleanupHubEntitiesHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl EventHandler for CleanupHubEntitiesHandler {
    async fn handle(&self, event: &AppEvent, pool: &PgPool) -> Result<(), AppError> {
        match event {
            AppEvent::Assistant(AssistantEvent::Deleted { assistant_id, .. }) => {
                tracing::info!(
                    "Cleaning up hub entities for deleted assistant: {}",
                    assistant_id
                );

                // Delete hub_entities record if it exists
                let result = sqlx::query!(
                    "DELETE FROM hub_entities WHERE entity_type = 'assistant' AND entity_id = $1",
                    assistant_id
                )
                .execute(pool)
                .await
                .map_err(AppError::database_error)?;

                if result.rows_affected() > 0 {
                    tracing::debug!("Deleted hub entity tracking for assistant {}", assistant_id);
                }

                Ok(())
            }
            AppEvent::McpServer(McpServerEvent::SystemServerDeleted { server_id })
            | AppEvent::McpServer(McpServerEvent::UserServerDeleted { server_id, .. }) => {
                tracing::info!(
                    "Cleaning up hub entities for deleted MCP server: {}",
                    server_id
                );

                // Delete hub_entities record if it exists
                let result = sqlx::query!(
                    "DELETE FROM hub_entities WHERE entity_type = 'mcp_server' AND entity_id = $1",
                    server_id
                )
                .execute(pool)
                .await
                .map_err(AppError::database_error)?;

                if result.rows_affected() > 0 {
                    tracing::debug!("Deleted hub entity tracking for MCP server {}", server_id);
                }

                Ok(())
            }
            _ => Ok(()), // Ignore other events
        }
    }

    fn handler_name(&self) -> &'static str {
        "HubModule::CleanupHubEntities"
    }
}
