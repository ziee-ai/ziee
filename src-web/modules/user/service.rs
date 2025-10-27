use super::models::*;
use super::repository::UserRepository;
use bcrypt::{hash, verify, DEFAULT_COST};
use rand::{rng, Rng};
use uuid::Uuid;

pub struct UserService {
    repository: UserRepository,
}

impl UserService {
    pub fn new(repository: UserRepository) -> Self {
        Self { repository }
    }

    /// Create a new user with password
    pub async fn create_user(
        &self,
        request: CreateUserRequest,
    ) -> Result<User, Box<dyn std::error::Error + Send + Sync>> {
        // Check if username already exists
        if let Some(_) = self.repository.get_by_username(&request.username).await? {
            return Err("Username already exists".into());
        }

        // Check if email already exists
        if let Some(_) = self.repository.get_by_email(&request.email).await? {
            return Err("Email already exists".into());
        }

        // Generate salt and hash password
        let salt = Self::generate_salt();
        let bcrypt_hash = hash(&request.password, DEFAULT_COST)?;

        let password_service = PasswordService {
            bcrypt: bcrypt_hash,
            salt,
        };

        // Create user
        let user = self
            .repository
            .create(
                &request.username,
                &request.email,
                &password_service,
                request.profile,
            )
            .await?;

        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<User>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.get_by_id(user_id).await?)
    }

    /// Get user by username
    #[allow(dead_code)]
    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.get_by_username(username).await?)
    }

    /// Get user by email
    #[allow(dead_code)]
    pub async fn get_user_by_email(
        &self,
        email: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.get_by_email(email).await?)
    }

    /// Update user
    pub async fn update_user(
        &self,
        user_id: Uuid,
        request: UpdateUserRequest,
    ) -> Result<Option<User>, Box<dyn std::error::Error + Send + Sync>> {
        // If updating username, check if it's already taken
        if let Some(ref username) = request.username {
            if let Some(existing_user) = self.repository.get_by_username(username).await? {
                if existing_user.id != user_id {
                    return Err("Username already exists".into());
                }
            }
        }

        Ok(self
            .repository
            .update(
                user_id,
                request.username.as_deref(),
                request.is_active,
                request.profile,
            )
            .await?)
    }

    /// Delete user
    pub async fn delete_user(
        &self,
        user_id: Uuid,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Check if user is protected
        if let Some(user) = self.repository.get_by_id(user_id).await? {
            if user.is_protected {
                return Err("Cannot delete protected user".into());
            }
        }

        Ok(self.repository.delete(user_id).await?)
    }

    /// List users with pagination
    pub async fn list_users(
        &self,
        page: i32,
        per_page: i32,
    ) -> Result<UserListResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (users, total) = self.repository.list(page, per_page).await?;

        Ok(UserListResponse {
            users,
            total,
            page,
            per_page,
        })
    }

    /// Change user password (requires old password)
    pub async fn change_password(
        &self,
        user_id: Uuid,
        request: ChangePasswordRequest,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Get user with services
        let user = self
            .repository
            .get_by_id(user_id)
            .await?
            .ok_or("User not found")?;

        // Verify old password
        let password_service = user
            .services
            .password
            .as_ref()
            .ok_or("User has no password service")?;

        if !verify(&request.old_password, &password_service.bcrypt)? {
            return Err("Invalid old password".into());
        }

        // Hash new password
        let salt = Self::generate_salt();
        let bcrypt_hash = hash(&request.new_password, DEFAULT_COST)?;

        let new_password_service = PasswordService {
            bcrypt: bcrypt_hash,
            salt,
        };

        Ok(self
            .repository
            .update_password(user_id, &new_password_service)
            .await?)
    }

    /// Reset user password (admin function, doesn't require old password)
    pub async fn reset_password(
        &self,
        user_id: Uuid,
        request: ResetPasswordRequest,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Hash new password
        let salt = Self::generate_salt();
        let bcrypt_hash = hash(&request.new_password, DEFAULT_COST)?;

        let new_password_service = PasswordService {
            bcrypt: bcrypt_hash,
            salt,
        };

        Ok(self
            .repository
            .update_password(user_id, &new_password_service)
            .await?)
    }

    /// Verify user password
    pub async fn verify_password(
        &self,
        username_or_email: &str,
        password: &str,
    ) -> Result<Option<User>, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get user by username first, then by email
        let user = if let Some(user) = self.repository.get_by_username(username_or_email).await? {
            user
        } else if let Some(user) = self.repository.get_by_email(username_or_email).await? {
            user
        } else {
            return Ok(None);
        };

        // Check if user is active
        if !user.is_active {
            return Err("User is not active".into());
        }

        // Verify password
        let password_service = user
            .services
            .password
            .as_ref()
            .ok_or("User has no password service")?;

        if !verify(password, &password_service.bcrypt)? {
            return Ok(None);
        }

        Ok(Some(user))
    }

    /// Generate random salt
    fn generate_salt() -> String {
        let mut rng = rng();
        let salt: String = (0..16)
            .map(|_| format!("{:02x}", rng.random::<u8>()))
            .collect();
        salt
    }
}
