//! Desktop Module Builder
//!
//! Creates and manages desktop modules.
//! Similar to server's app_builder.rs

use crate::module_api::DesktopModule;
use crate::modules::{
    auth::AuthModule, backend::BackendModule, host_mount::HostMountModule,
    magic_link::MagicLinkModule, remote_access::RemoteAccessModule, settings::SettingsModule,
    tray::TrayModule, tunnel_auth::TunnelAuthModule, updater::UpdaterModule,
};
use anyhow::Result;
use tauri::App;
use ziee::ApiRouter;

/// Create all desktop modules
///
/// This is where modules are registered. Add new modules here.
///
/// # Arguments
/// * `config_file` - Optional path to config file for backend module
pub fn create_desktop_modules(config_file: Option<String>) -> Vec<Box<dyn DesktopModule>> {
    vec![
        Box::new(BackendModule::new(config_file)),
        Box::new(AuthModule::new()),
        Box::new(SettingsModule::new()),
        Box::new(TrayModule::new()),
        Box::new(UpdaterModule::new()),
        // Remote-access tunnel + magic-link + tunnel-aware auth.
        // Order matters relative to BackendModule: BackendModule
        // sets up the pool + repo factory + JWT service; these
        // three must come after.
        Box::new(RemoteAccessModule::new()),
        Box::new(MagicLinkModule::new()),
        Box::new(TunnelAuthModule::new()),
        // Host-folder mounting into the code sandbox (feature #3, Part B).
        // After BackendModule so the pool/JWT exist; its API routes register
        // here, and its sandbox provider registers post-server-start in
        // backend/mod.rs.
        Box::new(HostMountModule::new()),
    ]
}

/// Initialize all modules
///
/// Called during app startup to initialize each module
pub fn initialize_modules(modules: &mut [Box<dyn DesktopModule>], app: &mut App) -> Result<()> {
    for module in modules.iter_mut() {
        tracing::info!("Initializing desktop module: {}", module.name());
        module.init(app)?;
        tracing::info!("Successfully initialized module: {}", module.name());
    }
    Ok(())
}

/// Build combined API router with OpenAPI documentation from all modules
///
/// Called to collect API routes that will be included in OpenAPI spec.
pub fn build_desktop_api_routes(modules: &[Box<dyn DesktopModule>]) -> ApiRouter {
    let mut router = ApiRouter::new();
    for module in modules.iter() {
        tracing::debug!("Collecting API routes from module: {}", module.name());
        router = module.register_api_routes(router);
    }
    tracing::info!("Desktop API routes collected from {} modules", modules.len());
    router
}
