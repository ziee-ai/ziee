// All feature modules
pub mod auth;
pub mod health;
pub mod user;
pub mod user_group;

// Re-export modules
pub use auth::AuthModule;
pub use health::HealthModule;
pub use user::UserModule;
pub use user_group::UserGroupModule;
