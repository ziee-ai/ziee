//! Window Module
//!
//! Window management functionality

pub mod commands;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;

pub struct WindowModule;

impl WindowModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for WindowModule {
    fn name(&self) -> &'static str {
        "window"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("Window module initialized");
        Ok(())
    }
}
