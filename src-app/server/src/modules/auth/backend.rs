// Auth backend implementation - currently unused but part of auth system infrastructure
// This module contains auth backend infrastructure that will be used when auth is fully implemented
#![allow(dead_code)]

use aide::OperationIo;
use axum_login::{AuthnBackend, UserId};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::user::User;

use super::password;

/// Type alias for AuthSession from axum-login
pub type AuthSession = axum_login::AuthSession<AuthBackend>;

/// Newtype wrapper for AuthSession to implement aide's OperationInput
/// This allows AuthSession to be used in aide-documented handlers
#[derive(OperationIo)]
#[aide(input)]
pub struct AuthSessionWrapper(pub AuthSession);

// Implement FromRequestParts so it can be used as an extractor
impl<S> axum::extract::FromRequestParts<S> for AuthSessionWrapper
where
    S: Send + Sync,
{
    type Rejection = <AuthSession as axum::extract::FromRequestParts<S>>::Rejection;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            <AuthSession as axum::extract::FromRequestParts<S>>::from_request_parts(parts, state)
                .await
                .map(AuthSessionWrapper)
        }
    }
}

// Deref to inner AuthSession for convenient access
impl std::ops::Deref for AuthSessionWrapper {
    type Target = AuthSession;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AuthSessionWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Credentials for authentication
#[derive(Debug, Clone)]
pub enum Credentials {
    /// Password-based authentication
    Password { username: String, password: String },
    /// Provider-based authentication (OAuth2, SAML, LDAP)
    Provider {
        provider_id: Uuid,
        external_id: String,
    },
}

/// Authentication backend for axum-login
#[derive(Clone)]
pub struct AuthBackend {
    pool: PgPool,
}

impl AuthBackend {
    /// Create a new authentication backend
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Authenticate with username and password
    async fn authenticate_password(
        &self,
        username: &str,
        password_input: &str,
    ) -> Result<Option<User>, AppError> {
        // Get user by username or email
        let user = Repos.user.get_by_username_or_email(username).await?;

        if let Some(user) = user {
            // Check if user is active
            if !user.is_active {
                return Err(AppError::unauthorized(
                    "ACCOUNT_DISABLED",
                    "User account is disabled",
                ));
            }

            // Check password
            if let Some(hash) = &user.password_hash {
                let valid = password::verify_password(password_input, hash).map_err(|e| {
                    AppError::internal_error(format!("Password verification error: {}", e))
                })?;

                if valid {
                    // Update last login
                    Repos.user.update_last_login(user.id).await?;
                    return Ok(Some(user));
                }
            } else {
                return Err(AppError::unauthorized(
                    "NO_PASSWORD",
                    "No password set for this user. Please use external authentication.",
                ));
            }
        }

        Ok(None)
    }

    /// Authenticate with provider-based credentials
    async fn authenticate_provider(
        &self,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Option<User>, AppError> {
        // Get user via auth link
        let link = sqlx::query!(
            r#"
            SELECT user_id
            FROM user_auth_links
            WHERE provider_id = $1 AND external_id = $2
            "#,
            provider_id,
            external_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        if let Some(link) = link {
            let user = Repos.user.get_by_id(link.user_id).await?;

            if let Some(user) = user {
                // Check if user is active
                if !user.is_active {
                    return Err(AppError::unauthorized(
                        "ACCOUNT_DISABLED",
                        "User account is disabled",
                    ));
                }

                // Update last login
                Repos.user.update_last_login(user.id).await?;

                // Update auth link last login
                sqlx::query!(
                    r#"
                    UPDATE user_auth_links
                    SET last_login_at = NOW()
                    WHERE provider_id = $1 AND external_id = $2
                    "#,
                    provider_id,
                    external_id
                )
                .execute(&self.pool)
                .await
                .map_err(AppError::database_error)?;

                return Ok(Some(user));
            }
        }

        Ok(None)
    }
}

impl AuthnBackend for AuthBackend {
    type User = User;
    type Credentials = Credentials;
    type Error = AppError;

    fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> impl std::future::Future<Output = Result<Option<Self::User>, Self::Error>> + Send {
        async move {
            match creds {
                Credentials::Password { username, password } => {
                    self.authenticate_password(&username, &password).await
                }
                Credentials::Provider {
                    provider_id,
                    external_id,
                } => self.authenticate_provider(provider_id, &external_id).await,
            }
        }
    }

    fn get_user(
        &self,
        user_id: &UserId<Self>,
    ) -> impl std::future::Future<Output = Result<Option<Self::User>, Self::Error>> + Send {
        let user_id = *user_id;
        async move {
            Repos.user.get_by_id(user_id).await
        }
    }
}

// Authorization methods (not using AuthzBackend trait for now)
impl AuthBackend {
    pub async fn get_all_permissions(&self, user: &User) -> Result<HashSet<String>, AppError> {
        let permissions = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT unnest(g.permissions) as "permission!"
            FROM groups g
            INNER JOIN user_groups ug ON ug.group_id = g.id
            WHERE ug.user_id = $1
            "#,
            user.id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(permissions.into_iter().collect())
    }

}
