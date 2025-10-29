// All feature modules
pub mod app;
pub mod auth;
pub mod hardware;
pub mod health;
pub mod llm_repository;
pub mod permissions;
pub mod user;

// Re-export modules
pub use app::AppModule;
pub use auth::AuthModule;
pub use health::HealthModule;
