// Project extension registration using linkme.
//
// Mirrors `modules/chat/extension_registration.rs`. Extensions self-register
// via the `PROJECT_EXTENSIONS` distributed slice and are initialized here
// in order based on their `order` value.
//
// Acid-test invariant: with zero extensions registered (all sibling
// modules deleted), this returns an empty registry. The project module's
// `register_routes` continues to work — extension routes simply contribute
// nothing.

use std::sync::Arc;
use sqlx::PgPool;

use crate::core::config::Config;
use crate::modules::project::core::extension::{PROJECT_EXTENSIONS, ProjectExtensionRegistry};

/// Register all discovered project extensions in order.
pub fn auto_register_project_extensions(
    pool: PgPool,
    config: Arc<Config>,
) -> ProjectExtensionRegistry {
    let mut registry = ProjectExtensionRegistry::new();

    let mut entries: Vec<_> = PROJECT_EXTENSIONS.iter().collect();
    entries.sort_by_key(|e| e.order);

    for entry in entries {
        tracing::debug!(
            "Registering project extension: {} (order: {})",
            entry.name,
            entry.order
        );
        let extension = (entry.factory)(pool.clone(), config.clone());
        registry.register(extension);
    }

    registry
}
