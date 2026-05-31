use sqlx::PgPool;

use crate::common::AppError;
use crate::modules::user::User;

/// App Repository
pub struct AppRepository {
    pool: PgPool,
}

impl AppRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create admin user with group assignments in a transaction
    /// Returns the created user
    pub async fn create_admin_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
        display_name: Option<String>,
    ) -> Result<User, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Double-check within transaction (race condition protection)
        let admin_exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM users WHERE is_admin = true) as "exists!""#
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        if admin_exists {
            tx.rollback().await.map_err(AppError::database_error)?;
            return Err(AppError::forbidden(
                "SETUP_ALREADY_COMPLETE",
                "Admin user already exists",
            ));
        }

        // Create admin user
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password_hash, display_name, is_active, is_admin)
            VALUES ($1, $2, $3, $4, true, true)
            RETURNING id, username, email, email_verified, password_hash, display_name,
                      avatar_url, is_active, is_admin, permissions, completed_onboarding_ids, completed_onboarding_step_ids,
                      created_at as "created_at: _", updated_at as "updated_at: _", last_login_at as "last_login_at: _",
                      password_changed_at as "password_changed_at: _"
            "#,
            username,
            email,
            password_hash,
            display_name
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        // Assign to Administrators group
        let admin_group =
            sqlx::query!(r#"SELECT id FROM groups WHERE name = 'Administrators' LIMIT 1"#)
                .fetch_one(&mut *tx)
                .await
                .map_err(AppError::database_error)?;

        sqlx::query!(
            r#"INSERT INTO user_groups (user_id, group_id) VALUES ($1, $2)"#,
            user.id,
            admin_group.id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        // Also assign to Users group (for access to default resources like MCP servers)
        let users_group = sqlx::query!(r#"SELECT id FROM groups WHERE name = 'Users' LIMIT 1"#)
            .fetch_one(&mut *tx)
            .await
            .map_err(AppError::database_error)?;

        sqlx::query!(
            r#"INSERT INTO user_groups (user_id, group_id) VALUES ($1, $2)"#,
            user.id,
            users_group.id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        // Commit transaction
        tx.commit().await.map_err(AppError::database_error)?;

        Ok(user)
    }
}
