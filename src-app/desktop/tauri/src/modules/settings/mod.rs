//! Settings Module
//!
//! Desktop-specific settings management

mod handlers;
mod routes;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;
use ziee_chat::Router;

pub struct SettingsModule;

impl SettingsModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for SettingsModule {
    fn name(&self) -> &'static str {
        "settings"
    }

    fn description(&self) -> &'static str {
        "Desktop-specific settings management"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Settings module initialized");
        Ok(())
    }

    fn register_routes(&self, router: Router) -> Router {
        tracing::info!("Registering settings routes");
        router.merge(routes::settings_routes())
    }
}
