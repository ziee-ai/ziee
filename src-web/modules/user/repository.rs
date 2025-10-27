use super::models::*;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get user by ID with all related data
    pub async fn get_by_id(&self, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let user_base = sqlx::query_as!(
            UserBase,
            r#"
            SELECT
                id,
                username,
                created_at,
                profile,
                is_active,
                is_protected,
                last_login_at,
                updated_at
            FROM users
            WHERE id = $1
            "#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(user_base) = user_base else {
            return Ok(None);
        };

        let emails = self.get_user_emails(user_id).await?;
        let services = self.get_user_services(user_id).await?;

        Ok(Some(User::from_db_parts(user_base, emails, services)))
    }

    /// Get user by username
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>, sqlx::Error> {
        let user_base = sqlx::query_as!(
            UserBase,
            r#"
            SELECT
                id,
                username,
                created_at,
                profile,
                is_active,
                is_protected,
                last_login_at,
                updated_at
            FROM users
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(user_base) = user_base else {
            return Ok(None);
        };

        let emails = self.get_user_emails(user_base.id).await?;
        let services = self.get_user_services(user_base.id).await?;

        Ok(Some(User::from_db_parts(user_base, emails, services)))
    }

    /// Get user by email
    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
        let user_email = sqlx::query_as!(
            UserEmail,
            r#"
            SELECT
                id,
                user_id,
                address,
                verified,
                created_at
            FROM user_emails
            WHERE address = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(user_email) = user_email else {
            return Ok(None);
        };

        self.get_by_id(user_email.user_id).await
    }

    /// Get all emails for a user
    async fn get_user_emails(&self, user_id: Uuid) -> Result<Vec<UserEmail>, sqlx::Error> {
        sqlx::query_as!(
            UserEmail,
            r#"
            SELECT
                id,
                user_id,
                address,
                verified,
                created_at
            FROM user_emails
            WHERE user_id = $1
            ORDER BY created_at
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get all services for a user
    async fn get_user_services(&self, user_id: Uuid) -> Result<Vec<UserService>, sqlx::Error> {
        sqlx::query_as!(
            UserService,
            r#"
            SELECT
                id,
                user_id,
                service_name,
                service_data,
                created_at
            FROM user_services
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Create a new user
    pub async fn create(
        &self,
        username: &str,
        email: &str,
        password_service: &PasswordService,
        profile: Option<serde_json::Value>,
    ) -> Result<User, sqlx::Error> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Insert user
        let user_base = sqlx::query_as!(
            UserBase,
            r#"
            INSERT INTO users (username, profile, is_active, is_protected)
            VALUES ($1, $2, true, false)
            RETURNING id, username, created_at, profile, is_active, is_protected, last_login_at, updated_at
            "#,
            username,
            profile
        )
        .fetch_one(&mut *tx)
        .await?;

        // Insert email
        let user_email = sqlx::query_as!(
            UserEmail,
            r#"
            INSERT INTO user_emails (user_id, address, verified)
            VALUES ($1, $2, false)
            RETURNING id, user_id, address, verified, created_at
            "#,
            user_base.id,
            email
        )
        .fetch_one(&mut *tx)
        .await?;

        // Insert password service
        let service_data = serde_json::to_value(password_service)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let user_service = sqlx::query_as!(
            UserService,
            r#"
            INSERT INTO user_services (user_id, service_name, service_data)
            VALUES ($1, 'password', $2)
            RETURNING id, user_id, service_name, service_data, created_at
            "#,
            user_base.id,
            service_data
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(User::from_db_parts(
            user_base,
            vec![user_email],
            vec![user_service],
        ))
    }

    /// Update user
    pub async fn update(
        &self,
        user_id: Uuid,
        username: Option<&str>,
        is_active: Option<bool>,
        profile: Option<serde_json::Value>,
    ) -> Result<Option<User>, sqlx::Error> {
        // Update only provided fields
        let user_base = if let Some(username) = username {
            sqlx::query_as!(
                UserBase,
                r#"
                UPDATE users
                SET username = COALESCE($2, username),
                    is_active = COALESCE($3, is_active),
                    profile = COALESCE($4, profile),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING id, username, created_at, profile, is_active, is_protected, last_login_at, updated_at
                "#,
                user_id,
                username,
                is_active,
                profile
            )
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as!(
                UserBase,
                r#"
                UPDATE users
                SET is_active = COALESCE($2, is_active),
                    profile = COALESCE($3, profile),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING id, username, created_at, profile, is_active, is_protected, last_login_at, updated_at
                "#,
                user_id,
                is_active,
                profile
            )
            .fetch_optional(&self.pool)
            .await?
        };

        let Some(user_base) = user_base else {
            return Ok(None);
        };

        let emails = self.get_user_emails(user_id).await?;
        let services = self.get_user_services(user_id).await?;

        Ok(Some(User::from_db_parts(user_base, emails, services)))
    }

    /// Delete user
    pub async fn delete(&self, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM users WHERE id = $1
            "#,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// List users with pagination
    pub async fn list(
        &self,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<User>, i64), sqlx::Error> {
        let offset = (page - 1) * per_page;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!" FROM users
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        // Get paginated users
        let user_bases = sqlx::query_as!(
            UserBase,
            r#"
            SELECT
                id,
                username,
                created_at,
                profile,
                is_active,
                is_protected,
                last_login_at,
                updated_at
            FROM users
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            per_page as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await?;

        // Build complete user objects
        let mut users = Vec::new();
        for user_base in user_bases {
            let emails = self.get_user_emails(user_base.id).await?;
            let services = self.get_user_services(user_base.id).await?;
            users.push(User::from_db_parts(user_base, emails, services));
        }

        Ok((users, total))
    }

    /// Update user password
    pub async fn update_password(
        &self,
        user_id: Uuid,
        password_service: &PasswordService,
    ) -> Result<bool, sqlx::Error> {
        let service_data = serde_json::to_value(password_service)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        let result = sqlx::query!(
            r#"
            UPDATE user_services
            SET service_data = $1
            WHERE user_id = $2 AND service_name = 'password'
            "#,
            service_data,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update last login timestamp
    pub async fn update_last_login(&self, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            UPDATE users
            SET last_login_at = NOW()
            WHERE id = $1
            "#,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Store login token
    pub async fn store_login_token(
        &self,
        user_id: Uuid,
        token: &str,
        when_created: i64,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<UserLoginToken, sqlx::Error> {
        sqlx::query_as!(
            UserLoginToken,
            r#"
            INSERT INTO user_login_tokens (user_id, token, when_created, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, token, when_created, expires_at, created_at
            "#,
            user_id,
            token,
            when_created,
            expires_at
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Get user by token
    pub async fn get_by_token(&self, token: &str) -> Result<Option<User>, sqlx::Error> {
        let login_token = sqlx::query_as!(
            UserLoginToken,
            r#"
            SELECT
                id,
                user_id,
                token,
                when_created,
                expires_at,
                created_at
            FROM user_login_tokens
            WHERE token = $1
            "#,
            token
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(login_token) = login_token else {
            return Ok(None);
        };

        // Check if token is expired
        if let Some(expires_at) = login_token.expires_at {
            if expires_at < chrono::Utc::now() {
                return Ok(None);
            }
        }

        self.get_by_id(login_token.user_id).await
    }

    /// Delete login token
    pub async fn delete_login_token(&self, token: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_login_tokens WHERE token = $1
            "#,
            token
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
