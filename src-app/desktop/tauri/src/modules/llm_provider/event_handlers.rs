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

#[ziee::async_trait]
impl ziee::EventHandler for AutoAssignProviderHandler {
    async fn handle(
        &self,
        event: &ziee::AppEvent,
        _pool: &sqlx::PgPool,
    ) -> std::result::Result<(), ziee::AppError> {
        if let ziee::AppEvent::LlmProvider(ziee::LlmProviderEvent::Created {
            provider,
        }) = event
        {
            tracing::info!(
                "Auto-assigning new provider '{}' to all groups",
                provider.name
            );

            if let Ok(groups) = ziee::Repos.group.get_all().await {
                let group_count = groups.len();
                for group in groups {
                    // bcc698d refactor split user-group↔provider mappings out
                    // of `llm_provider` into the `user_group_llm_provider` sub-
                    // repo. `assign_to_group` lives on the new repo now.
                    let _ = ziee::Repos
                        .user_group_llm_provider
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
