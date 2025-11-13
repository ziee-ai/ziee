// Auth provider infrastructure - part of future auth system
#![allow(dead_code)]

use sqlx::PgPool;

use super::models::{AuthProvider, OAuthSession};
use crate::common::AppError;

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
