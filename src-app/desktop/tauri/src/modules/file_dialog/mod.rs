//! File Dialog Module
//!
//! Native file picker dialogs

pub mod commands;

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::App;

pub struct FileDialogModule;

impl FileDialogModule {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopModule for FileDialogModule {
    fn name(&self) -> &'static str {
        "file_dialog"
    }

    fn init(&mut self, _app: &mut App) -> Result<()> {
        tracing::info!("File dialog module initialized");
        Ok(())
    }
}
