// Chat extension registration using linkme
//
// Extensions self-register using the CHAT_EXTENSIONS distributed slice
// and are initialized here in order based on their metadata.order value

use std::sync::Arc;
use sqlx::PgPool;

use crate::core::config::Config;
use crate::modules::chat::core::extension::{CHAT_EXTENSIONS, ExtensionRegistry};

/// Register all discovered extensions in order
pub fn auto_register_extensions(pool: PgPool, config: Arc<Config>) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();

    // Collect and sort extensions by order
    let mut entries: Vec<_> = CHAT_EXTENSIONS.iter().collect();
    entries.sort_by_key(|e| e.order);

    // Register each extension in order
    for entry in entries {
        tracing::debug!(
            "Registering chat extension: {} (order: {})",
            entry.name,
            entry.order
        );
        let extension = (entry.factory)(pool.clone(), config.clone());
        registry.register(extension);
    }

    registry
}
