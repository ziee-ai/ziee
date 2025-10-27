use super::models::*;
use crate::modules::user::{CreateUserRequest, UserRepository, UserService};
use chrono::{Duration, Utc};
use rand::{rng, Rng};

pub struct AuthService {
    user_repository: UserRepository,
    user_service: UserService,
}

impl AuthService {
    pub fn new(user_repository: UserRepository, user_service: UserService) -> Self {
        Self {
            user_repository,
            user_service,
        }
    }

    /// Login with username/email and password
    pub async fn login(
        &self,
        request: LoginRequest,
    ) -> Result<LoginResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Verify password
        let user = self
            .user_service
            .verify_password(&request.username_or_email, &request.password)
            .await?
            .ok_or("Invalid credentials")?;

        // Generate token
        let token = Self::generate_token();
        let when_created = Utc::now().timestamp_millis();
        let expires_at = Utc::now() + Duration::days(30);

        // Store token
        self.user_repository
            .store_login_token(user.id, &token, when_created, Some(expires_at))
            .await?;

        // Update last login
        self.user_repository.update_last_login(user.id).await?;

        Ok(LoginResponse {
            token,
            user: user.sanitized(),
            expires_at,
        })
    }

    /// Register a new user
    pub async fn register(
        &self,
        request: RegisterRequest,
    ) -> Result<LoginResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Create user
        let create_request = CreateUserRequest {
            username: request.username,
            email: request.email,
            password: request.password,
            profile: request.profile,
        };

        let user = self.user_service.create_user(create_request).await?;

        // Generate token
        let token = Self::generate_token();
        let when_created = Utc::now().timestamp_millis();
        let expires_at = Utc::now() + Duration::days(30);

        // Store token
        self.user_repository
            .store_login_token(user.id, &token, when_created, Some(expires_at))
            .await?;

        Ok(LoginResponse {
            token,
            user: user.sanitized(),
            expires_at,
        })
    }

    /// Logout (invalidate token)
    pub async fn logout(
        &self,
        token: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.user_repository.delete_login_token(token).await?)
    }

    /// Validate token and get user
    pub async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<crate::modules::user::User>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.user_repository.get_by_token(token).await?)
    }

    /// Get current user by token
    pub async fn get_current_user(
        &self,
        token: &str,
    ) -> Result<Option<crate::modules::user::User>, Box<dyn std::error::Error + Send + Sync>> {
        self.validate_token(token).await
    }

    /// Generate random token
    fn generate_token() -> String {
        let mut rng = rng();
        let token: String = (0..64)
            .map(|_| format!("{:02x}", rng.random::<u8>()))
            .collect();
        token
    }
}
