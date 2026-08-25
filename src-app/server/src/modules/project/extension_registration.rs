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
use crate::modules::project::core::extension::PROJECT_EXTENSIONS;
use crate::modules::project::ProjectExtensionRegistry;

/// Register all discovered project extensions in order.
///
/// Delegates the sort-by-order + factory-dispatch to the generic
/// `ziee_framework` primitive (gap G8) and wraps the result in the
/// project-specific `ProjectExtensionRegistry` newtype (which carries the
/// project fan-out methods). Extensions self-register via the
/// `PROJECT_EXTENSIONS` distributed slice — the project module never imports
/// them; an empty slice yields an empty registry (the acid-test invariant).
pub fn auto_register_project_extensions(
    pool: PgPool,
    config: Arc<Config>,
) -> ProjectExtensionRegistry {
    let mut registry = ProjectExtensionRegistry::new();
    for entry in ziee_framework::entity_extension::sorted_entries(&PROJECT_EXTENSIONS) {
        let extension = (entry.factory)(pool.clone(), config.clone());
        registry.register(extension);
    }
    registry
}
