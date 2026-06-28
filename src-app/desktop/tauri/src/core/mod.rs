//! Core Module
//!
//! Desktop application core functionality

pub mod app_handle;
pub mod module_builder;
pub mod repositories;
pub mod repository;

pub use app_handle::{get_app_handle, set_app_handle};
pub use module_builder::{build_desktop_api_routes, create_desktop_modules, initialize_modules};
pub use repository::{init_desktop_repositories, DesktopRepos};
