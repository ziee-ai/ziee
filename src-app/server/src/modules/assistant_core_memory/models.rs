use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, sqlx::FromRow)]
pub struct CoreMemoryBlock {
    pub id: Uuid,
    pub assistant_id: Uuid,
    pub user_id: Uuid,
    pub block_label: String,
    pub content: String,
    pub char_limit: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpsertCoreMemoryBlockRequest {
    pub assistant_id: Uuid,
    pub block_label: String,
    pub content: String,
    #[serde(default = "default_char_limit")]
    pub char_limit: i32,
}

fn default_char_limit() -> i32 {
    2000
}
