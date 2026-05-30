//! Runtime-settings DTOs.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct RuntimeSettings {
    pub idle_unload_secs: i32,
    pub auto_start_timeout_secs: i32,
    pub drain_timeout_secs: i32,
    pub allow_unsigned_downloads: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            idle_unload_secs: 1800,
            auto_start_timeout_secs: 30,
            drain_timeout_secs: 30,
            allow_unsigned_downloads: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateRuntimeSettingsRequest {
    pub idle_unload_secs: Option<i32>,
    pub auto_start_timeout_secs: Option<i32>,
    pub drain_timeout_secs: Option<i32>,
    pub allow_unsigned_downloads: Option<bool>,
}
