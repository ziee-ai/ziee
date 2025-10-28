// All feature modules
pub mod auth;
pub mod health;
pub mod permissions;
pub mod user;

// Re-export modules
pub use auth::AuthModule;
pub use health::HealthModule;
