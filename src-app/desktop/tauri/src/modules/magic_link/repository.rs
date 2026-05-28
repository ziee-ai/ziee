//! Database access for magic-link tokens.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use ziee::AppError;

use super::models::MagicLinkRow;

pub struct MagicLinkRepository {
    pool: PgPool,
}

impl MagicLinkRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a token row. `token_hash` is the SHA-256 hex of the
    /// plaintext token; the plaintext itself is never stored.
    ///
    /// We pass `expires_at` as a `DateTime<Utc>` and let Postgres do
    /// the cast via the `$3::timestamptz` annotation — the implicit
    /// sqlx-side OffsetDateTime requirement is bypassed.
    pub async fn insert(
        &self,
        token_hash: &str,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO magic_link_tokens (token_hash, user_id, expires_at)
            VALUES ($1, $2, $3::timestamptz)
            ON CONFLICT (token_hash) DO NOTHING
            "#,
            token_hash,
            user_id,
            expires_at as _,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Count un-used, un-expired tokens for a given user. Used by the
    /// issue handler to cap how many active links can exist at once
    /// (defense against a runaway UI looping on `/issue`).
    pub async fn count_active_for_user(&self, user_id: Uuid) -> Result<i64, AppError> {
        let n = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM magic_link_tokens
            WHERE user_id = $1
              AND used_at IS NULL
              AND expires_at > NOW()
            "#,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(n)
    }

    /// Look up a token by its SHA-256 hash. Returns None when absent
    /// OR expired OR already-used. Callers MUST treat None as
    /// "invalid" and return the same generic 401 (no enumeration).
    pub async fn consume(&self, token_hash: &str) -> Result<Option<MagicLinkRow>, AppError> {
        // Atomic compare-and-flip: only return the row if it's
        // unexpired AND unused, and in the same statement flip
        // used_at so a second exchange can't succeed.
        let row = sqlx::query_as!(
            MagicLinkRow,
            r#"
            UPDATE magic_link_tokens
            SET used_at = NOW()
            WHERE token_hash = $1
              AND used_at IS NULL
              AND expires_at > NOW()
            RETURNING token_hash, user_id,
                      expires_at as "expires_at: _",
                      used_at as "used_at: _",
                      created_at as "created_at: _"
            "#,
            token_hash,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Reaper: delete expired or used tokens older than 1 hour.
    /// Best-effort — caller should ignore errors.
    pub async fn reap_old(&self) -> Result<u64, AppError> {
        let res = sqlx::query!(
            r#"
            DELETE FROM magic_link_tokens
            WHERE expires_at < NOW() - INTERVAL '1 hour'
               OR (used_at IS NOT NULL AND used_at < NOW() - INTERVAL '1 hour')
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(res.rows_affected())
    }
}

/// SHA-256 hex digest of the plaintext token. Used as the DB primary
/// key — we never store the plaintext.
pub fn hash_token(plaintext: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_deterministic() {
        let h1 = hash_token("alpha-bravo-charlie");
        let h2 = hash_token("alpha-bravo-charlie");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_token_different_inputs_different_outputs() {
        let h1 = hash_token("alpha");
        let h2 = hash_token("bravo");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_token_64_hex_chars() {
        let h = hash_token("alpha");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
