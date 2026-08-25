// File module configuration access

use crate::core::config::JwtConfig;
use once_cell::sync::OnceCell;
use std::sync::Arc;

static JWT_CONFIG: OnceCell<Arc<JwtConfig>> = OnceCell::new();

/// Initialize file module JWT config (called once during module init)
pub fn init_jwt_config(config: Arc<JwtConfig>) {
    JWT_CONFIG.set(config).expect("JWT config already initialized");
}

/// Get JWT config for file downloads
pub fn get_jwt_config() -> &'static Arc<JwtConfig> {
    JWT_CONFIG.get().expect("JWT config not initialized")
}
