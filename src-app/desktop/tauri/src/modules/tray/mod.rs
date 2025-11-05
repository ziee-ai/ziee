//! System Tray Module
//!
//! System tray integration

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;

pub struct TrayModule;

impl TrayModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for TrayModule {
    fn name(&self) -> &'static str {
        "tray"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Tray module initialized (system tray implementation pending)");

        // System tray implementation for Tauri 2.x would go here
        // The API has changed significantly from Tauri 1.x
        // For now, this is a placeholder

        Ok(())
    }
}
