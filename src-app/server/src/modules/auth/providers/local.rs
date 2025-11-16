// Auth provider infrastructure - part of future auth system
#![allow(dead_code)]

use async_trait::async_trait;
use sqlx::PgPool;

use super::{AuthError, AuthProvider, AuthProviderTrait, AuthResult, UserAttributes};
use crate::core::Repos;
use crate::modules::auth::password;
use crate::modules::user::User;

/// Local authentication provider using database-stored passwords
pub struct LocalAuthProvider {
    name: String,
    config: serde_json::Value,
    pool: PgPool,
}

impl LocalAuthProvider {
    pub fn new(provider: &AuthProvider, pool: PgPool) -> Result<Self, AuthError> {
        Ok(Self {
            name: provider.name.clone(),
            config: provider.config.clone(),
            pool,
        })
    }

    async fn get_user(&self, username: &str) -> Result<Option<User>, AuthError> {
        // Try username first
        if let Some(user) = Repos.user
            .get_by_username(username)
            .await
            .map_err(|e| AuthError::InternalError(format!("Database error: {}", e)))?
        {
            return Ok(Some(user));
        }

        // Try email
        Repos.user.get_by_email(username)
            .await
            .map_err(|e| AuthError::InternalError(format!("Database error: {}", e)))
    }
}

#[async_trait]
impl AuthProviderTrait for LocalAuthProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &str {
        "local"
    }

    async fn authenticate(&self, username: &str, password: &str) -> Result<AuthResult, AuthError> {
        // Get user by username or email
        let user = self
            .get_user(username)
            .await?
            .ok_or_else(|| AuthError::InvalidCredentials("User not found".to_string()))?;

        // Check if user has password hash
        let password_hash = user.password_hash.as_ref().ok_or_else(|| {
            AuthError::InvalidCredentials("No password configured for this user".to_string())
        })?;

        // Verify password (password::verify_password uses bcrypt internally)
        let valid = password::verify_password(password, password_hash)
            .map_err(|e| AuthError::InternalError(format!("Password verification error: {}", e)))?;

        if !valid {
            return Err(AuthError::InvalidCredentials(
                "Invalid password".to_string(),
            ));
        }

        // Return auth result
        Ok(AuthResult {
            external_id: user.id.to_string(),
            external_username: Some(user.username.clone()),
            external_email: Some(user.email.clone()),
            metadata: serde_json::json!({
                "provider": "local",
                "auth_method": "password"
            }),
            attributes: UserAttributes {
                username: user.username.clone(),
                email: user.email.clone(),
                display_name: user.display_name.clone(),
                first_name: None,   // Not tracked separately in new schema
                last_name: None,    // Not tracked separately in new schema
                groups: Vec::new(), // TODO: Add group support
            },
        })
    }

    async fn test_connection(&self) -> Result<(), AuthError> {
        // For local provider, just verify database connectivity
        Repos.user.get_by_username("__test_connection__")
            .await
            .map_err(|e| {
                AuthError::ConnectionFailed(format!("Database connection failed: {}", e))
            })?;

        Ok(())
    }

    fn get_config(&self) -> &serde_json::Value {
        &self.config
    }
}
