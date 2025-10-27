use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// User group structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct UserGroup {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub permissions: serde_json::Value,
    pub is_protected: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserGroup {
    /// Get permissions as a Vec<String>
    #[allow(dead_code)]
    pub fn get_permissions(&self) -> Vec<String> {
        serde_json::from_value(self.permissions.clone()).unwrap_or_default()
    }

    /// Set permissions from Vec<String>
    #[allow(dead_code)]
    pub fn set_permissions(&mut self, permissions: Vec<String>) {
        self.permissions = serde_json::to_value(permissions).unwrap_or(serde_json::json!([]));
    }
}

// User group membership structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct UserGroupMembership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub group_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<Uuid>,
}

// API Request/Response structures
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateUserGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateUserGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssignUserToGroupRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserGroupListResponse {
    pub groups: Vec<UserGroup>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserGroupMembersResponse {
    pub memberships: Vec<UserGroupMembershipWithUser>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserGroupMembershipWithUser {
    pub id: Uuid,
    pub user_id: Uuid,
    pub group_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<Uuid>,
    pub username: String,
}
