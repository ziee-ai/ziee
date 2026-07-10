// Core chat module - Core chat functionality with extension system

pub mod ai_provider;
pub mod export;
pub mod extension;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod routes;
pub mod services;
pub mod types;

// Re-export for convenience
pub use repository::ChatRepository;

// Re-export DB entities from models

// Re-export API types from types

// Note: StreamContext is exported from extension module, not models
