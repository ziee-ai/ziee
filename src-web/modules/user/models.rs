use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Base User structure (for direct DB operations)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserBase {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub profile: Option<serde_json::Value>,
    pub is_active: bool,
    pub is_protected: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

// User service structure
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserService {
    pub id: Uuid,
    pub user_id: Uuid,
    pub service_name: String,
    pub service_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// User login token structure
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserLoginToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub when_created: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// Email structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, FromRow)]
pub struct UserEmail {
    pub id: Uuid,
    pub user_id: Uuid,
    pub address: String,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

// Password service structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PasswordService {
    pub bcrypt: String,
    pub salt: String,
}

// User services wrapper
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct UserServices {
    pub password: Option<PasswordService>,
}

// Complete User structure (for API responses)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub emails: Vec<UserEmail>,
    pub created_at: DateTime<Utc>,
    pub profile: Option<serde_json::Value>,
    pub services: UserServices,
    pub is_active: bool,
    pub is_protected: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Build User from database parts
    pub fn from_db_parts(
        user_base: UserBase,
        emails: Vec<UserEmail>,
        services: Vec<UserService>,
    ) -> Self {
        let mut user = User {
            id: user_base.id,
            username: user_base.username,
            emails,
            created_at: user_base.created_at,
            profile: user_base.profile,
            services: UserServices::default(),
            is_active: user_base.is_active,
            is_protected: user_base.is_protected,
            last_login_at: user_base.last_login_at,
            updated_at: user_base.updated_at,
        };

        // Build services from database records
        for service in services {
            match service.service_name.as_str() {
                "password" => {
                    if let Ok(pwd_service) =
                        serde_json::from_value::<PasswordService>(service.service_data)
                    {
                        user.services.password = Some(pwd_service);
                    }
                }
                _ => {}
            }
        }

        user
    }

    /// Create a sanitized version without sensitive data
    pub fn sanitized(mut self) -> Self {
        self.services = UserServices::default();
        self
    }

    /// Get primary email
    #[allow(dead_code)]
    pub fn get_primary_email(&self) -> Option<String> {
        self.emails.first().map(|e| e.address.clone())
    }
}

// API Request/Response structures
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub profile: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<bool>,
    pub profile: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserListResponse {
    pub users: Vec<User>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}
