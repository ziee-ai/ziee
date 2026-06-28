use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::config::JwtConfig;

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // Subject (user ID)
    pub exp: i64,         // Expiration time
    pub iat: i64,         // Issued at
    pub iss: String,      // Issuer
    pub aud: String,      // Audience
    pub username: String, // Username
    pub email: String,    // Email
    pub is_admin: bool,   // Admin flag
    /// JWT ID — populated only on refresh tokens (used for the whitelist
    /// lookup in modules/auth/refresh_tokens.rs that closes 01-auth F-02
    /// + F-03). Optional + default so existing tests that hand-mint
    /// claims without a jti continue to deserialize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

/// JWT token pair (access + refresh)
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// TokenPair + the refresh token's jti + expires_at.
///
/// Returned by JwtService::generate_tokens so the caller can register
/// the refresh-token row in the `refresh_tokens` whitelist before the
/// token is handed back to the user. See the comment on generate_tokens
/// for the two-step protocol that closes 01-auth F-02 + F-03.
#[derive(Debug)]
pub struct TokenPairWithJti {
    pub pair: TokenPair,
    pub refresh_jti: Uuid,
    pub refresh_expires_at: chrono::DateTime<Utc>,
}

/// JWT service for token generation and validation
#[derive(Clone)]
pub struct JwtService {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

/// Minimum acceptable JWT secret length (bytes). HMAC-SHA256 ideally
/// uses ≥ 32 bytes of entropy. Closes 01-auth F-10 + 14-core F-03.
const MIN_JWT_SECRET_LEN: usize = 32;

/// Known shipped placeholder secrets. Refuse to boot with any of these
/// — if an operator's config still contains a template value they
/// almost certainly forgot to override it, and the secret is in public
/// source control. Plain string match, not substring, so genuine
/// 32+-char operator secrets aren't accidentally rejected.
const BANNED_JWT_SECRETS: &[&str] = &[
    "dev-secret-change-in-production-min-32-chars-long",
    "REPLACE_ME_WITH_A_LONG_RANDOM_SECRET_AT_LEAST_32_CHARS",
    "your-secret-key-here",
    "change-me",
    "secret",
    "changeme",
];

impl JwtService {
    /// Create a new JWT service. Errors if the secret is shorter than
    /// MIN_JWT_SECRET_LEN bytes or matches a known shipped placeholder.
    /// Closes 01-auth F-10 + 14-core F-03 (weak/default JWT secret
    /// accepted at runtime). Callers (main.rs, lib.rs) propagate the
    /// error so the server refuses to boot rather than continuing with
    /// a weak signer.
    pub fn try_new(config: JwtConfig) -> Result<Self, AppError> {
        if config.secret.len() < MIN_JWT_SECRET_LEN {
            return Err(AppError::internal_error(format!(
                "JWT secret is {} bytes; minimum is {}. Set jwt.secret in \
                 your config to a random ≥32-char string (e.g. \
                 `openssl rand -base64 48`).",
                config.secret.len(),
                MIN_JWT_SECRET_LEN
            )));
        }
        if BANNED_JWT_SECRETS.iter().any(|p| *p == config.secret) {
            return Err(AppError::internal_error(
                "JWT secret matches a shipped placeholder value. Set \
                 jwt.secret in your config to a unique random ≥32-char \
                 string (e.g. `openssl rand -base64 48`).",
            ));
        }

        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
        })
    }

    /// Infallible constructor preserved for tests / callers that have
    /// already validated the secret. Production code MUST use `try_new`
    /// so a weak secret aborts boot. This thin wrapper panics on a bad
    /// secret so misuse can't go unnoticed.
    ///
    /// Used cross-crate by the `ziee-desktop` integration tests, so it
    /// appears unused from the `ziee` crate's own build — keep it.
    #[allow(dead_code)]
    pub fn new(config: JwtConfig) -> Self {
        Self::try_new(config).expect("JWT secret validation failed; use try_new for graceful errors")
    }

    /// Generate access and refresh tokens for a user (legacy form).
    ///
    /// Refresh token does NOT carry a jti, so it bypasses the whitelist
    /// check in the refresh handler (no jti → no whitelist gate). Kept
    /// for callers that don't yet wire refresh-token registration. New
    /// code should use `generate_tokens_with_jti` and register the
    /// returned jti in the `refresh_tokens` table — that's the path
    /// that closes 01-auth F-02 + F-03.
    pub fn generate_tokens(
        &self,
        user_id: Uuid,
        username: &str,
        email: &str,
        is_admin: bool,
    ) -> Result<TokenPair, AppError> {
        let access_token = self.generate_access_token(user_id, username, email, is_admin)?;
        let refresh_token = self.generate_legacy_refresh_token(user_id)?;
        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_expiry_hours * 3600,
        })
    }

    /// Legacy refresh token without a jti — used by callers that
    /// haven't yet wired the whitelist.
    fn generate_legacy_refresh_token(&self, user_id: Uuid) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = now + Duration::days(self.config.refresh_token_expiry_days);

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: self.config.issuer.clone(),
            aud: format!("{}-refresh", self.config.audience),
            username: String::new(),
            email: String::new(),
            is_admin: false,
            jti: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            AppError::internal_error(format!("Failed to generate refresh token: {}", e))
        })
    }

    /// Generate access and refresh tokens for a user.
    ///
    /// Returns the TokenPair plus the refresh token's `jti` and
    /// `expires_at`. The caller (handler) MUST then write a row to
    /// `refresh_tokens` so the new refresh token is whitelisted; without
    /// that follow-up write, the whitelist check will reject the
    /// freshly-issued token. The two-step protocol (mint then register)
    /// is deliberate — it lets callers fail closed if the DB write fails,
    /// without minting a usable secret. Closes 01-auth F-02 + F-03 once
    /// the handlers wire register/revoke.
    pub fn generate_tokens_with_jti(
        &self,
        user_id: Uuid,
        username: &str,
        email: &str,
        is_admin: bool,
    ) -> Result<TokenPairWithJti, AppError> {
        let access_token = self.generate_access_token(user_id, username, email, is_admin)?;
        let (refresh_token, refresh_jti, refresh_expires_at) =
            self.generate_refresh_token_with_jti(user_id)?;

        Ok(TokenPairWithJti {
            pair: TokenPair {
                access_token,
                refresh_token,
                token_type: "Bearer".to_string(),
                expires_in: self.config.access_token_expiry_hours * 3600,
            },
            refresh_jti,
            refresh_expires_at,
        })
    }

    /// Generate an access token
    fn generate_access_token(
        &self,
        user_id: Uuid,
        username: &str,
        email: &str,
        is_admin: bool,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.config.access_token_expiry_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            username: username.to_string(),
            email: email.to_string(),
            is_admin,
            jti: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            AppError::internal_error(format!("Failed to generate access token: {}", e))
        })
    }

    /// Generate a refresh token (simpler claims, longer expiry, carries
    /// a jti for whitelist tracking).
    ///
    /// Returns (token, jti, expires_at). Callers must register the jti
    /// in the `refresh_tokens` table — see generate_tokens for the
    /// two-step protocol.
    fn generate_refresh_token_with_jti(
        &self,
        user_id: Uuid,
    ) -> Result<(String, Uuid, chrono::DateTime<Utc>), AppError> {
        let now = Utc::now();
        let exp = now + Duration::days(self.config.refresh_token_expiry_days);
        let jti = Uuid::new_v4();

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: self.config.issuer.clone(),
            aud: format!("{}-refresh", self.config.audience),
            username: String::new(),
            email: String::new(),
            is_admin: false,
            jti: Some(jti.to_string()),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            AppError::internal_error(format!("Failed to generate refresh token: {}", e))
        })?;

        Ok((token, jti, exp))
    }

    /// Validate and decode an access token
    pub fn validate_access_token(&self, token: &str) -> Result<Claims, AppError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| {
                AppError::unauthorized("INVALID_TOKEN", format!("Invalid or expired token: {}", e))
            })
    }

    /// Validate and decode a refresh token
    pub fn validate_refresh_token(&self, token: &str) -> Result<Claims, AppError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&format!("{}-refresh", self.config.audience)]);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| {
                AppError::unauthorized(
                    "INVALID_REFRESH_TOKEN",
                    format!("Invalid or expired refresh token: {}", e),
                )
            })
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(auth_header: &str) -> Result<&str, AppError> {
        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::unauthorized(
                "INVALID_AUTH_HEADER",
                "Authorization header must start with 'Bearer '",
            ));
        }

        let token = &auth_header[7..];
        if token.is_empty() {
            return Err(AppError::unauthorized(
                "MISSING_TOKEN",
                "Token is missing from Authorization header",
            ));
        }

        Ok(token)
    }
}
