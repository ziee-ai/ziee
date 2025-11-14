// Shared types for modules
// Week 1, Day 1

use linkme::distributed_slice;

pub use super::backend_module::AppModule;

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
