//! Desktop Module Builder
//!
//! Creates and manages desktop modules.
//! Similar to server's app_builder.rs

use crate::module_api::DesktopModule;
use crate::modules::{
    backend::BackendModule, file_dialog::FileDialogModule, tray::TrayModule,
    updater::UpdaterModule, window::WindowModule,
};
use anyhow::Result;
use tauri::App;

/// Create all desktop modules
///
/// This is where modules are registered. Add new modules here.
pub fn create_desktop_modules() -> Vec<Box<dyn DesktopModule>> {
    vec![
        Box::new(BackendModule::new()),
        Box::new(WindowModule::new()),
        Box::new(TrayModule::new()),
        Box::new(FileDialogModule::new()),
        Box::new(UpdaterModule::new()),
    ]
}

/// Initialize all modules
///
/// Called during app startup to initialize each module
pub fn initialize_modules(
    modules: &mut [Box<dyn DesktopModule>],
    app: &mut App,
) -> Result<()> {
    for module in modules.iter_mut() {
        tracing::info!("Initializing desktop module: {}", module.name());
        module.init(app)?;
        tracing::info!("Successfully initialized module: {}", module.name());
    }
    Ok(())
}
