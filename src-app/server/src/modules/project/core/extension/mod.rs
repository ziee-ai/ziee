// Extension system for the project module.
//
// Mirrors `modules/chat/core/extension/`. See `registry.rs` for the trait
// definition and the acid-test invariant.

pub mod metadata;
pub mod registry;

pub use registry::{
    PROJECT_EXTENSIONS, ProjectExtension, ProjectExtensionEntry, ProjectExtensionRegistry,
    get_global_registry, set_global_registry,
};
