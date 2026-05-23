//! Whitelist + revocation tracking for refresh tokens.
//!
//! Backs the closure of 01-auth F-02 (logout was a no-op) and F-03
//! (refresh didn't rotate the presented token). See migration 44 for
//! the schema rationale.

use crate::common::AppError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Insert an active row for a freshly-issued refresh token. Call this
/// immediately AFTER JwtService::generate_tokens_with_jti so the token
/// is whitelisted before being returned to the user.
pub async fn register(
    pool: &PgPool,
    jti: Uuid,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    // sqlx uses time::OffsetDateTime for TIMESTAMPTZ; convert via Unix
    // seconds (chrono and time share the same instant model).
    let expires_at_ts = time::OffsetDateTime::from_unix_timestamp(expires_at.timestamp())
        .map_err(|e| AppError::internal_error(format!("invalid expires_at: {}", e)))?;
    sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (jti, user_id, expires_at)
        VALUES ($1, $2, $3)
        "#,
        jti,
        user_id,
        expires_at_ts,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Return true iff the row for this jti exists with revoked_at IS NULL
/// and expires_at > NOW(). Called as the gate on every refresh attempt.
pub async fn is_active(pool: &PgPool, jti: Uuid) -> Result<bool, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT 1 as "exists!"
        FROM refresh_tokens
        WHERE jti = $1 AND revoked_at IS NULL AND expires_at > NOW()
        "#,
        jti,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.is_some())
}

/// Mark a single jti as revoked. Used during rotation (the old refresh
/// token is revoked at the moment the new pair is minted).
pub async fn revoke(pool: &PgPool, jti: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE refresh_tokens SET revoked_at = NOW() WHERE jti = $1"#,
        jti,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Revoke every active refresh token belonging to `user_id`. Used by
/// logout (the audit's F-02: logout was a no-op; now it actually signs
/// the user out by killing every refresh token they hold).
pub async fn revoke_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = NOW()
        WHERE user_id = $1 AND revoked_at IS NULL
        "#,
        user_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}
