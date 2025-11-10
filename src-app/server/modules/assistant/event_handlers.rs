// Event handlers for assistant module
// Handles application events related to assistants

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::AppError;
use crate::core::events::{AppEvent, EventHandler};
use crate::modules::user::events::UserEvent;
use super::{repository, models};

/// Clones enabled default template assistants to newly created users
pub struct CloneTemplateAssistantsHandler;

impl CloneTemplateAssistantsHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl EventHandler for CloneTemplateAssistantsHandler {
    async fn handle(&self, event: &AppEvent, pool: &PgPool) -> Result<(), AppError> {
        match event {
            AppEvent::User(UserEvent::Created { user }) => {
                tracing::info!(
                    "Cloning default template assistants for new user: {} ({})",
                    user.username,
                    user.id
                );

                // Get all template assistants
                let templates = repository::list_assistants(
                    pool,
                    None,
                    true,  // Only templates
                    1,
                    100,   // Get up to 100 templates
                ).await?;

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

                        let request = models::CreateAssistantRequest {
                            name: template.name.clone(),
                            description: template.description.clone(),
                            instructions: template.instructions.clone(),
                            parameters,
                            is_template: Some(false),
                            is_default: Some(template.is_default),
                            enabled: Some(template.enabled),
                            source: Some(models::AssistantSource::Template {
                                id: template.id.to_string()
                            }),
                        };

                        match repository::create_assistant(
                            pool,
                            Some(user.id),
                            request,
                        ).await {
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
