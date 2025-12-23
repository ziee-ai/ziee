//! LLM Provider Event Handlers
//!
//! Handles provider-related events for the desktop app

use std::sync::Arc;

/// Event handler that auto-assigns new LLM providers to all user groups.
/// This ensures users can access all providers in the desktop app.
pub struct AutoAssignProviderHandler;

impl AutoAssignProviderHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[ziee_chat::async_trait]
impl ziee_chat::EventHandler for AutoAssignProviderHandler {
    async fn handle(
        &self,
        event: &ziee_chat::AppEvent,
        _pool: &sqlx::PgPool,
    ) -> std::result::Result<(), ziee_chat::AppError> {
        if let ziee_chat::AppEvent::LlmProvider(ziee_chat::LlmProviderEvent::Created {
            provider,
        }) = event
        {
            tracing::info!(
                "Auto-assigning new provider '{}' to all groups",
                provider.name
            );

            if let Ok(groups) = ziee_chat::Repos.group.get_all().await {
                let group_count = groups.len();
                for group in groups {
                    let _ = ziee_chat::Repos
                        .llm_provider
                        .assign_to_group(provider.id, group.id)
                        .await;
                }
                tracing::debug!(
                    "Provider '{}' assigned to {} groups",
                    provider.name,
                    group_count
                );
            }
        }
        Ok(())
    }

    fn handler_name(&self) -> &'static str {
        "Desktop::AutoAssignProvider"
    }
}
