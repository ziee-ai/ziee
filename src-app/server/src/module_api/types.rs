// Shared types for modules
// Week 1, Day 1

use linkme::distributed_slice;

pub use super::backend_module::AppModule;

/// Module metadata for automatic registration and ordering
///
/// Each module should define a METADATA constant with this type:
/// ```
/// pub const METADATA: ModuleMetadata = ModuleMetadata {
///     name: "module_name",
///     order: 10,
///     description: "Module description",
/// };
/// ```
///
/// The order field determines the registration order:
/// - Lower numbers are registered first
/// - Default order is 50 if not specified
/// - Common ranges:
///   - 0-20: Core infrastructure modules (auth, user, permissions)
///   - 20-40: Service modules (llm_provider, llm_model)
///   - 40-60: Application modules (chat, assistant)
///   - 60-80: Extension modules (mcp, hub)
///   - 80-100: UI/API modules (health, app)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ModuleMetadata {
    pub name: &'static str,
    pub order: i32,
    pub description: &'static str,
}

/// Module registration entry combining metadata with constructor
///
/// Use the `#[distributed_slice(MODULE_ENTRIES)]` attribute to register modules:
/// ```
/// #[distributed_slice(crate::module_api::MODULE_ENTRIES)]
/// static USER_MODULE: ModuleEntry = ModuleEntry {
///     name: "user",
///     order: 10,
///     description: "User and group management",
///     constructor: || Box::new(UserModule::new()),
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ModuleEntry {
    pub name: &'static str,
    pub order: i32,
    pub description: &'static str,
    pub constructor: fn() -> Box<dyn AppModule>,
}

/// Distributed slice for module registrations
///
/// Modules register themselves by adding to this slice using #[distributed_slice]
#[distributed_slice]
pub static MODULE_ENTRIES: [ModuleEntry] = [..];
