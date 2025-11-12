// Core chat module - Core chat functionality with extension system

pub mod extension;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod services;

// Re-export for convenience
pub use extension::*;
pub use handlers::*;
pub use models::*;
pub use permissions::*;
pub use repository::*;
pub use routes::*;
pub use services::*;
