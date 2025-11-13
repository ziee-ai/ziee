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
}

/// JWT token pair (access + refresh)
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// JWT service for token generation and validation
#[derive(Clone)]
pub struct JwtService {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }

    /// Generate access and refresh tokens for a user
    pub fn generate_tokens(
        &self,
        user_id: Uuid,
        username: &str,
        email: &str,
        is_admin: bool,
    ) -> Result<TokenPair, AppError> {
        let access_token = self.generate_access_token(user_id, username, email, is_admin)?;
        let refresh_token = self.generate_refresh_token(user_id)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_expiry_hours * 3600,
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
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            AppError::internal_error(format!("Failed to generate access token: {}", e))
        })
    }

    /// Generate a refresh token (simpler claims, longer expiry)
    fn generate_refresh_token(&self, user_id: Uuid) -> Result<String, AppError> {
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
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            AppError::internal_error(format!("Failed to generate refresh token: {}", e))
        })
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
