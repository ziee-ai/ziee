use sqlx::PgPool;
use uuid::Uuid;

use super::models::{Group, User};
use crate::common::AppError;

// =====================================================
// User Repository
// =====================================================

#[derive(Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get user by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, email_verified, password_hash, display_name,
                   avatar_url, is_active, is_admin, permissions,
                   created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            FROM users
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get user by username
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, email_verified, password_hash, display_name,
                   avatar_url, is_active, is_admin, permissions,
                   created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            FROM users
            WHERE username = $1
            "#,
            username
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get user by email
    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, email_verified, password_hash, display_name,
                   avatar_url, is_active, is_admin, permissions,
                   created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            FROM users
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get user by username or email
    pub async fn get_by_username_or_email(&self, identifier: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, email_verified, password_hash, display_name,
                   avatar_url, is_active, is_admin, permissions,
                   created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            FROM users
            WHERE username = $1 OR email = $1
            "#,
            identifier
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// List users with pagination
    pub async fn list(&self, page: i32, per_page: i32) -> Result<(Vec<User>, i64), AppError> {
        let offset = ((page - 1) * per_page) as i64;

        // Get total count
        let total: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM users"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        // Get paginated users
        let users = sqlx::query_as!(
            User,
            r#"
            SELECT id, username, email, email_verified, password_hash, display_name,
                   avatar_url, is_active, is_admin, permissions,
                   created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            FROM users
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            per_page as i64,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok((users, total))
    }

    /// Create a new user
    pub async fn create(
        &self,
        username: &str,
        email: &str,
        password_hash: Option<String>,
        display_name: Option<String>,
        permissions: Option<Vec<String>>,
    ) -> Result<User, AppError> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password_hash, display_name, permissions)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, username, email, email_verified, password_hash, display_name,
                      avatar_url, is_active, is_admin, permissions,
                      created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            "#,
            username,
            email,
            password_hash,
            display_name,
            permissions.as_deref().unwrap_or(&[])
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Check if an admin user exists
    pub async fn has_admin(&self) -> Result<bool, AppError> {
        let result = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM users WHERE is_admin = true) as "exists!""#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(result)
    }

    /// Create an admin user (only for initial setup)
    pub async fn create_admin(
        &self,
        username: &str,
        email: &str,
        password_hash: String,
        display_name: Option<String>,
    ) -> Result<User, AppError> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password_hash, display_name, is_active, is_admin)
            VALUES ($1, $2, $3, $4, true, true)
            RETURNING id, username, email, email_verified, password_hash, display_name,
                      avatar_url, is_active, is_admin, permissions,
                      created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            "#,
            username,
            email,
            password_hash,
            display_name
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Update user
    pub async fn update(
        &self,
        id: Uuid,
        username: Option<String>,
        email: Option<String>,
        display_name: Option<String>,
        permissions: Option<Vec<String>>,
    ) -> Result<User, AppError> {
        sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET username = COALESCE($2, username),
                email = COALESCE($3, email),
                display_name = COALESCE($4, display_name),
                permissions = COALESCE($5, permissions),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, username, email, email_verified, password_hash, display_name,
                      avatar_url, is_active, is_admin, permissions,
                      created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _"
            "#,
            id,
            username,
            email,
            display_name,
            permissions.as_deref()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Update password hash
    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE users
            SET password_hash = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            password_hash
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Update last login timestamp
    pub async fn update_last_login(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE users
            SET last_login_at = NOW()
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Set user active status
    pub async fn set_active(&self, id: Uuid, is_active: bool) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE users
            SET is_active = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            is_active
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Delete user
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM users WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Get user's groups
    pub async fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT g.id, g.name, g.description, g.permissions, g.is_system, g.is_active, g.is_default,
                   g.created_at as "created_at: _", g.updated_at as "updated_at: _"
            FROM groups g
            INNER JOIN user_groups ug ON ug.group_id = g.id
            WHERE ug.user_id = $1
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Assign user to group
    pub async fn assign_to_group(
        &self,
        user_id: Uuid,
        group_id: Uuid,
        assigned_by: Option<Uuid>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO user_groups (user_id, group_id, assigned_by)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, group_id) DO NOTHING
            "#,
            user_id,
            group_id,
            assigned_by
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Remove user from group
    pub async fn remove_from_group(&self, user_id: Uuid, group_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM user_groups
            WHERE user_id = $1 AND group_id = $2
            "#,
            user_id,
            group_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }
}

// =====================================================
// Group Repository
// =====================================================

#[derive(Clone)]
pub struct GroupRepository {
    pool: PgPool,
}

impl GroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get group by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get group by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get all groups
    pub async fn get_all(&self) -> Result<Vec<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Get default group (where is_default = true)
    pub async fn get_default(&self) -> Result<Option<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            WHERE is_default = true AND is_active = true
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// List groups with pagination
    pub async fn list(&self, page: i32, per_page: i32) -> Result<(Vec<Group>, i64), AppError> {
        let offset = (page - 1) * per_page;

        // Get total count
        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM groups")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::database_error)?
            .unwrap_or(0);

        // Get paginated results
        let groups = sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            ORDER BY name
            LIMIT $1 OFFSET $2
            "#,
            per_page as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok((groups, total))
    }

    /// Create a new group
    pub async fn create(
        &self,
        name: &str,
        description: Option<String>,
        permissions: Vec<String>,
    ) -> Result<Group, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            INSERT INTO groups (name, description, permissions)
            VALUES ($1, $2, $3)
            RETURNING id, name, description, permissions, is_system, is_active, is_default,
                      created_at as "created_at: _", updated_at as "updated_at: _"
            "#,
            name,
            description,
            &permissions
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Update group
    pub async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        description: Option<String>,
        permissions: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> Result<Group, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            UPDATE groups
            SET name = COALESCE($2, name),
                description = COALESCE($3, description),
                permissions = COALESCE($4, permissions),
                is_active = COALESCE($5, is_active),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, permissions, is_system, is_active, is_default,
                      created_at as "created_at: _", updated_at as "updated_at: _"
            "#,
            id,
            name,
            description,
            permissions.as_deref(),
            is_active
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Delete group (only non-system groups)
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            DELETE FROM groups WHERE id = $1 AND is_system = FALSE
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Get members of a group with pagination
    pub async fn get_members(
        &self,
        group_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<User>, i64), AppError> {
        let offset = (page - 1) * per_page;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM user_groups WHERE group_id = $1
            "#,
            group_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);

        // Get paginated members
        let users = sqlx::query_as!(
            User,
            r#"
            SELECT u.id, u.username, u.email, u.email_verified, u.password_hash,
                   u.display_name, u.avatar_url, u.is_active, u.is_admin,
                   ARRAY[]::TEXT[] as "permissions!",
                   u.created_at as "created_at: _", u.updated_at as "updated_at: _",
                   u.last_login_at as "last_login_at: _"
            FROM users u
            INNER JOIN user_groups ug ON u.id = ug.user_id
            WHERE ug.group_id = $1
            ORDER BY u.username
            LIMIT $2 OFFSET $3
            "#,
            group_id,
            per_page as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok((users, total))
    }
}
