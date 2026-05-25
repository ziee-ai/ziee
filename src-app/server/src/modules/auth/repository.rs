use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::auth::providers::models::{OAuthSession, PendingAccountLink};
use crate::modules::user::Group;

/// Auth Repository
pub struct AuthRepository {
    pool: PgPool,
}

impl AuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the default group
    pub async fn get_default_group(&self) -> Result<Option<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            WHERE is_default = true
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Assign user to default group
    pub async fn assign_user_to_default_group(&self, user_id: Uuid) -> Result<(), AppError> {
        let default_group = self.get_default_group().await?;

        if let Some(group) = default_group {
            sqlx::query!(
                r#"
                INSERT INTO user_groups (user_id, group_id, assigned_at)
                VALUES ($1, $2, NOW())
                "#,
                user_id,
                group.id
            )
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        }

        Ok(())
    }

    /// Find user auth link by provider and external ID
    pub async fn find_user_by_auth_link(
        &self,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let result = sqlx::query!(
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

        Ok(result.map(|r| r.user_id))
    }

    /// Create a user auth link
    pub async fn create_auth_link(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO user_auth_links (user_id, provider_id, external_id, created_at, last_login_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            "#,
            user_id,
            provider_id,
            external_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Create a user auth link including the provider's email + raw
    /// claims. Used by the social-login provisioning + First-Broker-Link
    /// flows. Use this in preference to the bare `create_auth_link`
    /// when you have the email/claims at hand.
    pub async fn create_auth_link_with_data(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
        external_id: &str,
        external_email: Option<&str>,
        external_data: Option<&serde_json::Value>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO user_auth_links (user_id, provider_id, external_id, external_email, external_data, created_at, last_login_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            "#,
            user_id,
            provider_id,
            external_id,
            external_email,
            external_data,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Bump `last_login_at` on an existing user_auth_links row.
    /// Called whenever a returning user re-authenticates via the
    /// social provider — distinct from the `users.last_login_at`
    /// bump because a user may have multiple linked providers.
    pub async fn update_auth_link_last_login(
        &self,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE user_auth_links
            SET last_login_at = NOW(), updated_at = NOW()
            WHERE provider_id = $1 AND external_id = $2
            "#,
            provider_id,
            external_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Find a local user by email — used to detect First-Broker-Login
    /// collisions. Returns the user_id if a local-password account
    /// exists with the given email. NOTE: matches on the literal
    /// email; callers can lowercase first if they want
    /// case-insensitive behavior.
    pub async fn find_user_by_email_for_linking(
        &self,
        email: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT id FROM users WHERE email = $1 LIMIT 1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.id))
    }

    /// Create a new user from external auth (used by LDAP/OAuth)
    /// Returns the created user's ID
    pub async fn create_external_user(
        &self,
        username: &str,
        email: Option<String>,
        display_name: &str,
    ) -> Result<Uuid, AppError> {
        let new_user_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, display_name, is_active, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())
            "#,
            new_user_id,
            username,
            email,
            display_name
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(new_user_id)
    }

    /// Create external user with auth link and assign to default group
    /// This is a convenience method that combines multiple operations
    pub async fn create_external_user_with_link(
        &self,
        username: &str,
        email: Option<String>,
        display_name: &str,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Uuid, AppError> {
        let user_id = self
            .create_external_user(username, email, display_name)
            .await?;

        self.create_auth_link(user_id, provider_id, external_id)
            .await?;

        self.assign_user_to_default_group(user_id).await?;

        Ok(user_id)
    }

    /// Create OAuth session for OAuth/OIDC flows
    pub async fn create_oauth_session(&self, session: &OAuthSession) -> Result<(), AppError> {
        let expires_at_timestamp = session.expires_at.timestamp() as f64;
        sqlx::query!(
            r#"
            INSERT INTO oauth_sessions (id, state, provider_id, pkce_verifier, nonce, redirect_uri, return_to, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, to_timestamp($8))
            "#,
            session.id,
            session.state,
            session.provider_id,
            session.pkce_verifier,
            session.nonce,
            session.redirect_uri,
            session.return_to,
            expires_at_timestamp
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Get OAuth session by state
    pub async fn get_oauth_session_by_state(
        &self,
        state: &str,
    ) -> Result<Option<OAuthSession>, AppError> {
        sqlx::query_as!(
            OAuthSession,
            r#"
            SELECT id, state, provider_id, pkce_verifier, nonce, redirect_uri, return_to,
                   created_at as "created_at: _",
                   expires_at as "expires_at: _"
            FROM oauth_sessions
            WHERE state = $1 AND expires_at > NOW()
            "#,
            state
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Delete OAuth session by state
    pub async fn delete_oauth_session(&self, state: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM oauth_sessions
            WHERE state = $1
            "#,
            state
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Create a pending account link with a 10-minute TTL. The
    /// returned token is what we put in the `/auth/link-account?token=...`
    /// redirect URL.
    pub async fn create_pending_link(
        &self,
        provider_id: Uuid,
        target_user_id: Uuid,
        external_id: &str,
        external_email: Option<&str>,
        external_data: Option<&serde_json::Value>,
    ) -> Result<String, AppError> {
        let link_token = Uuid::new_v4().to_string();
        let expires_at: DateTime<Utc> = Utc::now() + Duration::minutes(10);
        let expires_at_ts = expires_at.timestamp() as f64;
        sqlx::query!(
            r#"
            INSERT INTO pending_account_links (link_token, provider_id, target_user_id, external_id, external_email, external_data, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7))
            "#,
            link_token,
            provider_id,
            target_user_id,
            external_id,
            external_email,
            external_data,
            expires_at_ts,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(link_token)
    }

    /// Atomically read + delete a pending link by token. Returns
    /// None if the token is unknown or expired. Single-use: a
    /// subsequent call with the same token returns None.
    pub async fn consume_pending_link(
        &self,
        link_token: &str,
    ) -> Result<Option<PendingAccountLink>, AppError> {
        sqlx::query_as!(
            PendingAccountLink,
            r#"
            DELETE FROM pending_account_links
            WHERE link_token = $1 AND expires_at > NOW()
            RETURNING link_token, provider_id, target_user_id, external_id,
                      external_email, external_data,
                      created_at as "created_at: _",
                      expires_at as "expires_at: _"
            "#,
            link_token
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }
}
