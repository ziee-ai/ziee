//! Database access for the remote_access module.
//!
//! Singleton row at `remote_access_settings.id = 1`, pre-created by
//! migration 10000000000003 (desktop #3). CRUD operations:
//!   - `get_settings()` — read + decrypt
//!   - `update_settings()` — patch fields, encrypt token if provided
//!
//! Encryption uses the shared `crate::common::encrypt_secret`
//! / `decrypt_secret` helpers (pgcrypto pgp_sym_*). Storage key comes
//! from `ziee::storage_key()`.

use ziee::{AppError, decrypt_secret, encrypt_secret, resolve_optional_secret};
use sqlx::PgPool;

use super::models::{RemoteAccessSettings, RemoteAccessSettingsRow};

pub struct RemoteAccessRepository {
    pool: PgPool,
}

impl RemoteAccessRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the singleton settings row and decrypt the auth token (if
    /// stored). Returns the in-memory `RemoteAccessSettings` view.
    /// Errors only on DB / decryption failure — the row is
    /// guaranteed to exist by migration 10000000000003 (desktop #3).
    pub async fn get_settings(&self) -> Result<RemoteAccessSettings, AppError> {
        let row = self.get_row().await?;

        let had_ciphertext = row.ngrok_auth_token_enc.is_some();
        let token =
            resolve_optional_secret(&self.pool, row.ngrok_auth_token_enc, None).await;

        // resolve_optional_secret returns None on BOTH "no ciphertext"
        // and "decrypt failed" — log when the ciphertext WAS present
        // but came back None so a key rotation / pgcrypto MAC mismatch
        // / corrupted storage_key doesn't masquerade as "no token".
        if had_ciphertext && token.is_none() {
            tracing::warn!(
                "remote_access: stored ngrok_auth_token failed to decrypt. \
                 Most likely cause: secrets.storage_key changed between writes. \
                 The admin will need to re-save the token in the Remote Access page."
            );
        }

        Ok(RemoteAccessSettings {
            ngrok_auth_token: token,
            ngrok_domain: row.ngrok_domain,
            auto_start_tunnel: row.auto_start_tunnel,
            password_auth_enabled: row.password_auth_enabled,
        })
    }

    /// Read the raw row (for the GET endpoint that needs timestamps +
    /// the auth_token_set bool without decrypting the token itself).
    pub async fn get_row(&self) -> Result<RemoteAccessSettingsRow, AppError> {
        sqlx::query_as!(
            RemoteAccessSettingsRow,
            r#"
            SELECT id, ngrok_auth_token_enc, ngrok_domain, auto_start_tunnel,
                   password_auth_enabled,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM remote_access_settings
            WHERE id = 1
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Update fields. Three-state semantics:
    ///   - `None` argument → don't touch
    ///   - `Some(None)` → clear (NULL / FALSE)
    ///   - `Some(Some(v))` → set
    ///
    /// Returns the post-update row.
    pub async fn update_settings(
        &self,
        token_patch: Option<Option<String>>,
        domain_patch: Option<Option<String>>,
        auto_start_patch: Option<bool>,
        password_auth_patch: Option<bool>,
    ) -> Result<RemoteAccessSettingsRow, AppError> {
        // Encrypt token if a new value was provided. None → leave
        // column alone; Some(None) → clear; Some(Some(v)) → encrypt+store.
        let encrypted_token: Option<Option<Vec<u8>>> = match token_patch {
            None => None,
            Some(None) => Some(None),
            Some(Some(plaintext)) => {
                let key = ziee::storage_key();
                let enc = encrypt_secret(&self.pool, &plaintext, key).await?;
                // encrypt_secret returns Ok(None) only when no storage key
                // is configured. In that mode we refuse to store secrets.
                let enc = enc.ok_or_else(|| {
                    AppError::internal_error(
                        "Cannot persist ngrok auth token: secrets.storage_key is not configured. \
                         Set it in the server config before enabling remote access.",
                    )
                })?;
                Some(Some(enc))
            }
        };

        // Build the UPDATE with COALESCE-style "preserve if None" for
        // each tri-state column. Easier with explicit cases per
        // column since sqlx macros want compile-time-known SQL.
        let row = sqlx::query_as!(
            RemoteAccessSettingsRow,
            r#"
            UPDATE remote_access_settings
            SET
                ngrok_auth_token_enc = CASE
                    WHEN $1::INT = 0 THEN ngrok_auth_token_enc       -- untouched
                    WHEN $1::INT = 1 THEN NULL                       -- explicit clear
                    ELSE $2::BYTEA                                   -- set
                END,
                ngrok_domain = CASE
                    WHEN $3::INT = 0 THEN ngrok_domain
                    WHEN $3::INT = 1 THEN NULL
                    ELSE $4::TEXT
                END,
                auto_start_tunnel = COALESCE($5::BOOLEAN, auto_start_tunnel),
                password_auth_enabled = COALESCE($6::BOOLEAN, password_auth_enabled),
                updated_at = NOW()
            WHERE id = 1
            RETURNING id, ngrok_auth_token_enc, ngrok_domain, auto_start_tunnel,
                      password_auth_enabled,
                      created_at as "created_at: _",
                      updated_at as "updated_at: _"
            "#,
            tri_state_marker(&encrypted_token),
            encrypted_token.flatten(),
            tri_state_marker(&domain_patch),
            domain_patch.flatten(),
            auto_start_patch,
            password_auth_patch,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(row)
    }
}

/// Map the tri-state Option<Option<T>> to an integer marker for SQL:
///   0 = untouched
///   1 = clear (set NULL)
///   2 = set to a value (use the value parameter)
fn tri_state_marker<T>(patch: &Option<Option<T>>) -> i32 {
    match patch {
        None => 0,
        Some(None) => 1,
        Some(Some(_)) => 2,
    }
}
