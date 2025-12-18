//! System Tray Module
//!
//! System tray integration for Tauri 2.x

use crate::module_api::DesktopModule;
use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App, Manager,
};

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

    fn description(&self) -> &'static str {
        "System tray integration"
    }

    fn init(&mut self, app: &mut App) -> Result<()> {
        tracing::info!("Initializing system tray...");

        // Create menu items
        let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
        let hide_item = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;
        let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
        let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        // Create menu
        let menu = Menu::with_items(app, &[&show_item, &hide_item, &separator, &quit_item])?;

        // Build tray icon
        let _tray = TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| {
                let id = event.id.as_ref();
                tracing::debug!("Tray menu event: {}", id);

                match id {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        tracing::info!("Quit requested from tray menu");
                        app.exit(0);
                    }
                    _ => {
                        tracing::warn!("Unknown tray menu item: {}", id);
                    }
                }
            })
            .on_tray_icon_event(|tray, event| {
                use tauri::tray::TrayIconEvent;
                if let TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left,
                    button_state: tauri::tray::MouseButtonState::Up,
                    ..
                } = event
                {
                    // Show window on left click
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            })
            .build(app)?;

        tracing::info!("System tray initialized successfully");
        Ok(())
    }
}
