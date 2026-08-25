// Module API — re-export shim over `ziee-framework`'s module system (Chunk B2).
//
// `AppModule` / `ModuleContext` / `ModuleEntry` / `MODULE_ENTRIES` moved into
// `ziee-framework`; ziee re-exports them here so every module's
// `use crate::module_api::{…}` + `#[distributed_slice(MODULE_ENTRIES)]`
// registration site is unchanged and links into the framework's one slice.

pub use ziee_framework::{AppModule, ModuleContext, ModuleEntry, MODULE_ENTRIES};

use crate::core::config::Config;
use std::sync::Arc;

/// Recover the app's full monolithic [`Config`] from a [`ModuleContext`].
///
/// The framework `ModuleContext` carries only `ServerConfig` (postgresql /
/// server / logging / jwt) in `ctx.config`; a module that needs a domain
/// sub-config (voice / code_sandbox / update_check / …) or the whole `Config`
/// (chat/project extension registries, scheduler) reads it here. ziee always
/// injects `Arc<Config>` into `app_config`, so the downcast never fails.
pub fn app_config(ctx: &ModuleContext) -> Arc<Config> {
    ctx.app_config
        .clone()
        .downcast::<Config>()
        .expect("ModuleContext.app_config must be ziee's Config")
}
