// Event handlers for assistant module
// Handles application events related to assistants

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use super::types;
use crate::common::AppError;
use crate::core::events::{AppEvent, EventHandler};
use crate::core::Repos;
use crate::modules::user::events::UserEvent;

/// Clones enabled default template assistants to newly created users
pub struct CloneTemplateAssistantsHandler;

impl CloneTemplateAssistantsHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl EventHandler for CloneTemplateAssistantsHandler {
    async fn handle(&self, event: &AppEvent, _pool: &PgPool) -> Result<(), AppError> {
        match event {
            AppEvent::User(UserEvent::Created { user }) => {
                tracing::info!(
                    "Cloning default template assistants for new user: {} ({})",
                    user.username,
                    user.id
                );

                // Get all template assistants
                let templates = Repos.assistant.list(
                    None, true, // Only templates
                    1, 100, // Get up to 100 templates
                )
                .await?;

                let mut cloned_count = 0;

                // Clone each default template to the user
                for template in templates.assistants {
                    if template.is_default && template.enabled {
                        // Parse parameters from template
                        let parameters = match template.get_parameters() {
                            Ok(params) => params,
                            Err(e) => {
                                tracing::error!(
                                    "Failed to parse parameters for template '{}': {}",
                                    template.name,
                                    e
                                );
                                continue; // Skip this template
                            }
                        };

                        let request = types::CreateAssistantRequest {
                            name: template.name.clone(),
                            description: template.description.clone(),
                            instructions: template.instructions.clone(),
                            parameters,
                            is_template: Some(false),
                            // Closes 10-assistant F-04 (Medium): the
                            // original cloned the template's
                            // is_default=true verbatim, so every new
                            // user signup minted a forced-default
                            // assistant the user couldn't opt out of
                            // at signup time. Default to false; the
                            // user explicitly picks their default
                            // post-signup via the UI.
                            is_default: Some(false),
                            enabled: Some(template.enabled),
                        };

                        match Repos.assistant.create(Some(user.id), request).await {
                            Ok(_) => {
                                cloned_count += 1;
                                tracing::debug!(
                                    "Cloned template '{}' to user {}",
                                    template.name,
                                    user.id
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to clone template '{}' to user {}: {}",
                                    template.name,
                                    user.id,
                                    e
                                );
                                // Continue with other templates even if one fails
                            }
                        }
                    }
                }

                tracing::info!(
                    "Cloned {} default template assistant(s) to user {}",
                    cloned_count,
                    user.id
                );

                Ok(())
            }
            _ => Ok(()), // Ignore other events
        }
    }

    fn handler_name(&self) -> &'static str {
        "AssistantModule::CloneTemplateAssistants"
    }
}
