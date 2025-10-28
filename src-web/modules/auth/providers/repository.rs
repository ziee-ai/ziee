use sqlx::PgPool;
use uuid::Uuid;

use super::models::{AuthProvider, OAuthSession, UserAuthLink};
use crate::common::AppError;

/// Get all enabled auth providers
pub async fn get_enabled_providers(pool: &PgPool) -> Result<Vec<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE enabled = TRUE
        ORDER BY name ASC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// Get auth provider by ID
pub async fn get_provider_by_id(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Option<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE id = $1
        "#,
        provider_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// Get auth provider by name
pub async fn get_provider_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<AuthProvider>, AppError> {
    sqlx::query_as!(
        AuthProvider,
        r#"
        SELECT id, name, provider_type, enabled, config,
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM auth_providers
        WHERE name = $1
        "#,
        name
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// Create OAuth session for OAuth/OIDC flows
pub async fn create_oauth_session(
    pool: &PgPool,
    session: &OAuthSession,
) -> Result<(), AppError> {
    let expires_at_timestamp = session.expires_at.timestamp() as f64;
    sqlx::query!(
        r#"
        INSERT INTO oauth_sessions (id, state, provider_id, pkce_verifier, nonce, redirect_uri, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7))
        "#,
        session.id,
        session.state,
        session.provider_id,
        session.pkce_verifier,
        session.nonce,
        session.redirect_uri,
        expires_at_timestamp
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

/// Get OAuth session by state
pub async fn get_oauth_session_by_state(
    pool: &PgPool,
    state: &str,
) -> Result<Option<OAuthSession>, AppError> {
    sqlx::query_as!(
        OAuthSession,
        r#"
        SELECT id, state, provider_id, pkce_verifier, nonce, redirect_uri,
               created_at as "created_at: _",
               expires_at as "expires_at: _"
        FROM oauth_sessions
        WHERE state = $1 AND expires_at > NOW()
        "#,
        state
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// Delete OAuth session by state
pub async fn delete_oauth_session(
    pool: &PgPool,
    state: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        DELETE FROM oauth_sessions
        WHERE state = $1
        "#,
        state
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

/// Clean up expired OAuth sessions
pub async fn cleanup_expired_oauth_sessions(pool: &PgPool) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM oauth_sessions
        WHERE expires_at < NOW()
        "#
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected())
}

/// Get user auth link by provider and external ID
pub async fn get_user_auth_link_by_provider_and_external_id(
    pool: &PgPool,
    provider_id: Uuid,
    external_id: &str,
) -> Result<Option<UserAuthLink>, AppError> {
    sqlx::query_as!(
        UserAuthLink,
        r#"
        SELECT id, user_id, provider_id, external_id, external_email, external_data,
               last_login_at as "last_login_at: _",
               created_at as "created_at: _",
               updated_at as "updated_at: _"
        FROM user_auth_links
        WHERE provider_id = $1 AND external_id = $2
        "#,
        provider_id,
        external_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)
}

/// Create user auth link
pub async fn create_user_auth_link(
    pool: &PgPool,
    user_id: Uuid,
    provider_id: Uuid,
    external_id: &str,
    external_email: Option<String>,
    external_data: Option<serde_json::Value>,
) -> Result<UserAuthLink, AppError> {
    sqlx::query_as!(
        UserAuthLink,
        r#"
        INSERT INTO user_auth_links (user_id, provider_id, external_id, external_email, external_data)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, user_id, provider_id, external_id, external_email, external_data,
                  last_login_at as "last_login_at: _",
                  created_at as "created_at: _",
                  updated_at as "updated_at: _"
        "#,
        user_id,
        provider_id,
        external_id,
        external_email,
        external_data
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)
}

/// Update user auth link data
pub async fn update_user_auth_link_data(
    pool: &PgPool,
    link_id: Uuid,
    external_email: Option<String>,
    external_data: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE user_auth_links
        SET external_email = $2,
            external_data = $3,
            updated_at = NOW()
        WHERE id = $1
        "#,
        link_id,
        external_email,
        external_data
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

/// Update user auth link last login timestamp
pub async fn update_user_auth_link_last_login(
    pool: &PgPool,
    link_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE user_auth_links
        SET last_login_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
        link_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}
