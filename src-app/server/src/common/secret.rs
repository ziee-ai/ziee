//! Secret-handling primitives for the application.
//!
//! Two layers:
//!
//! 1. `SecretView<T>` — type-safe wrapper for secrets in memory. The
//!    `Serialize` impl always emits `"<redacted>"` so accidentally
//!    returning a `SecretView<String>` in a JSON response can't leak the
//!    value. Callers that genuinely need the plaintext (outbound HTTP
//!    clients) call `.expose_secret()`.
//!
//! 2. `encrypt_secret` / `decrypt_secret` — pgcrypto round-trip helpers
//!    used by repository write/read paths. Uses pgp_sym_encrypt with
//!    the storage_key from `SecretsConfig`. Storage is `bytea`; in-memory
//!    is `SecretView<String>`.
//!
//! Closes the cross-cutting plaintext-secret-storage finding the audit
//! flagged in 06-llm-provider F-02 (Critical), 09-llm-repository F-02
//! (High), and adjacent items.

use crate::common::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

const MIN_STORAGE_KEY_LENGTH: usize = 32;

/// Wrapper around a secret value that refuses to serialize the plaintext.
///
/// The inner type is intentionally `String` for now — full generic
/// `T: Serialize` is overkill for the current use cases (api_key, token,
/// password). Add the generic form when a real second type shows up.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct SecretView<T> {
    inner: T,
}

impl<T> SecretView<T> {
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }

    /// Return a reference to the plaintext. Use sparingly — only at the
    /// boundary where the secret needs to leave the process (outbound
    /// HTTP header, subprocess argv, etc.).
    pub fn expose_secret(&self) -> &T {
        &self.inner
    }

    /// Consume the wrapper and return the plaintext.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Serialize for SecretView<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("<redacted>")
    }
}

// Note: no `JsonSchema` impl. If SecretView ever needs to appear in a
// JsonSchema-derived DTO, the recommendation is to wrap a `String` field
// at the boundary rather than baking SecretView into the schema —
// JsonSchema is for response shape, and SecretView's response shape is
// always the `<redacted>` literal which is well-modeled as `String`.

/// Encrypt a plaintext secret to a bytea blob using pgcrypto's
/// pgp_sym_encrypt. Returns Ok(Some(bytes)) on success, Ok(None) when
/// the storage key is unset (compat mode for unconfigured deployments).
/// Errors only on actual DB / encryption failure.
pub async fn encrypt_secret(
    pool: &PgPool,
    plaintext: &str,
    storage_key: Option<&str>,
) -> Result<Option<Vec<u8>>, AppError> {
    let key = match storage_key {
        Some(k) if k.len() >= MIN_STORAGE_KEY_LENGTH => k,
        Some(_) => {
            return Err(AppError::internal_error(
                "Secret storage key configured but shorter than 32 chars",
            ));
        }
        None => return Ok(None),
    };

    let row: (Vec<u8>,) = sqlx::query_as("SELECT pgp_sym_encrypt($1, $2)")
        .bind(plaintext)
        .bind(key)
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?;

    Ok(Some(row.0))
}

/// Decrypt a bytea blob to plaintext using pgcrypto's pgp_sym_decrypt.
/// Returns Ok(plaintext) on success. Errors on DB / decryption failure
/// (typically a wrong key — fail closed and surface a generic error to
/// the caller).
pub async fn decrypt_secret(
    pool: &PgPool,
    ciphertext: &[u8],
    storage_key: &str,
) -> Result<String, AppError> {
    if storage_key.len() < MIN_STORAGE_KEY_LENGTH {
        return Err(AppError::internal_error(
            "Secret storage key shorter than 32 chars",
        ));
    }

    let row: (String,) = sqlx::query_as("SELECT pgp_sym_decrypt($1, $2)")
        .bind(ciphertext)
        .bind(storage_key)
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?;

    Ok(row.0)
}

/// Repository-side resolver for the encrypted/plaintext dual-column
/// pattern. Prefers the encrypted column when storage_key is available
/// and decryption succeeds; otherwise falls back to the plaintext
/// column (compat mode + not-yet-backfilled rows).
///
/// Logs a `tracing::error!` if decryption fails (wrong key / corrupted
/// data) but does NOT propagate the error — the caller gets the
/// plaintext fallback (which is None on freshly-encrypted rows whose
/// plaintext column was cleared on write, so the secret simply
/// becomes unavailable rather than 500-ing every read).
pub async fn resolve_optional_secret(
    pool: &PgPool,
    encrypted: Option<Vec<u8>>,
    plaintext: Option<String>,
) -> Option<String> {
    if let Some(ref enc) = encrypted {
        if let Some(key) = crate::core::secrets::storage_key() {
            match decrypt_secret(pool, enc, key).await {
                Ok(plain) => return Some(plain),
                Err(e) => {
                    tracing::error!(
                        error = ?e,
                        "Failed to decrypt secret column; falling back to plaintext column \
                         (likely wrong storage_key or corrupted ciphertext)"
                    );
                }
            }
        }
    }
    plaintext
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_view_serializes_redacted() {
        let s = SecretView::new("sk-real-api-key-12345".to_string());
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"<redacted>\"");
        assert!(!json.contains("sk-real"));
    }

    #[test]
    fn secret_view_expose_returns_real_value() {
        let s = SecretView::new("plaintext".to_string());
        assert_eq!(s.expose_secret(), "plaintext");
    }

    #[test]
    fn secret_view_in_struct_redacts() {
        #[derive(Serialize)]
        struct Provider {
            name: String,
            api_key: SecretView<String>,
        }
        let p = Provider {
            name: "OpenAI".to_string(),
            api_key: SecretView::new("sk-secret".to_string()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("sk-secret"));
        assert!(json.contains("<redacted>"));
        assert!(json.contains("OpenAI"));
    }
}
