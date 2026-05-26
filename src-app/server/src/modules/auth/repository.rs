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
    /// collisions. Case-insensitive (LOWER(email)) so a user who
    /// registered as `Bob@corp.com` is matched when an OAuth provider
    /// hands back the canonical `bob@corp.com`. Without this, FBL is
    /// bypassed silently and the user gets a duplicate account.
    ///
    /// SECURITY: filters on `is_active = true`. A disabled user's
    /// email would otherwise trigger the FBL flow → /auth/link-account
    /// page renders → attacker learns the email is registered. The
    /// auto-provision branch creates a fresh account instead (which
    /// can later be reconciled if/when the original account is
    /// reactivated).
    pub async fn find_user_by_email_for_linking(
        &self,
        email: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT id FROM users
            WHERE LOWER(email) = LOWER($1) AND is_active = true
            LIMIT 1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.id))
    }

    /// Atomically provision a brand-new external-only user (no
    /// password) + bind the social identity + assign the default
    /// group, in a single transaction. If any step fails, the whole
    /// thing rolls back — without this, a partial failure leaves
    /// orphan rows that lock the user out forever (no password →
    /// can't local-login, no auth_link → can't social-login,
    /// email-collision check on retry refuses to provision).
    /// Returns the new user_id.
    pub async fn provision_external_user_atomic(
        &self,
        username: &str,
        email: Option<&str>,
        display_name: &str,
        provider_id: Uuid,
        external_id: &str,
        external_data: Option<&serde_json::Value>,
    ) -> Result<Uuid, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let new_user_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, display_name, is_active, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())
            "#,
            new_user_id, username, email, display_name,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        sqlx::query!(
            r#"
            INSERT INTO user_auth_links (user_id, provider_id, external_id, external_email, external_data, created_at, last_login_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            "#,
            new_user_id, provider_id, external_id, email, external_data,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        // Assign default group — fetch within the same tx so we see
        // a consistent snapshot.
        let default_group = sqlx::query!(
            r#"SELECT id FROM groups WHERE is_default = true LIMIT 1"#
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        if let Some(group) = default_group {
            sqlx::query!(
                r#"INSERT INTO user_groups (user_id, group_id, assigned_at) VALUES ($1, $2, NOW())"#,
                new_user_id, group.id,
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(new_user_id)
    }

    /// Atomic SELECT + UPDATE on user_auth_links: bump last_login_at
    /// and return the user_id in a single round-trip. Replaces the
    /// prior SELECT-then-UPDATE pattern in oauth_callback.
    pub async fn touch_auth_link_and_get_user_id(
        &self,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"
            UPDATE user_auth_links
            SET last_login_at = NOW(), updated_at = NOW()
            WHERE provider_id = $1 AND external_id = $2
            RETURNING user_id
            "#,
            provider_id, external_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.user_id))
    }

    /// Peek a pending link by token WITHOUT consuming it. Used in
    /// `link_account` so a wrong-password attempt doesn't burn the
    /// single-use token — the user gets to retry without re-running
    /// the whole OAuth flow.
    pub async fn peek_pending_link(
        &self,
        link_token: &str,
    ) -> Result<Option<crate::modules::auth::providers::models::PendingAccountLink>, AppError> {
        sqlx::query_as!(
            crate::modules::auth::providers::models::PendingAccountLink,
            r#"
            SELECT link_token, provider_id, target_user_id, external_id,
                   external_email, external_data, attempts,
                   created_at as "created_at: _",
                   expires_at as "expires_at: _"
            FROM pending_account_links
            WHERE link_token = $1 AND expires_at > NOW()
            "#,
            link_token
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Atomically increment `attempts` and return the new value. Used
    /// in `link_account` to enforce a per-token attempt cap: at the
    /// global 5 req/s rate limit a single IP could try ~3000 passwords
    /// in the 10-minute TTL; this gate cuts that to single digits and
    /// makes brute-forcing impractical even from a botnet.
    pub async fn bump_pending_link_attempts(
        &self,
        link_token: &str,
    ) -> Result<Option<i32>, AppError> {
        let row = sqlx::query!(
            r#"
            UPDATE pending_account_links
               SET attempts = attempts + 1
             WHERE link_token = $1 AND expires_at > NOW()
            RETURNING attempts
            "#,
            link_token,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.attempts))
    }

    /// Delete a pending link by token (best-effort — no error if
    /// the row's already gone). Paired with `peek_pending_link`
    /// when single-use semantics need to be enforced after the
    /// password verification step succeeds.
    pub async fn delete_pending_link(
        &self,
        link_token: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"DELETE FROM pending_account_links WHERE link_token = $1"#,
            link_token,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
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

    /// Best-effort cleanup of expired oauth_sessions + pending_account_links
    /// rows. Designed to be called from a periodic background task or
    /// at server boot; safe to invoke at any time. Returns
    /// `(sessions_pruned, pending_links_pruned)` counts.
    ///
    /// Even with the per-row TTL columns, rows we never re-touch (a
    /// user who abandons the OAuth dance mid-flow) would otherwise
    /// accumulate forever — both tables would grow without bound.
    pub async fn cleanup_expired_auth_rows(&self) -> Result<(u64, u64), AppError> {
        let s = sqlx::query!(
            r#"DELETE FROM oauth_sessions WHERE expires_at < NOW()"#
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        let p = sqlx::query!(
            r#"DELETE FROM pending_account_links WHERE expires_at < NOW()"#
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok((s.rows_affected(), p.rows_affected()))
    }
}
