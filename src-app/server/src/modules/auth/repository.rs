use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::user::Group;

/// Auth Repository
pub struct AuthRepository {
    pool: PgPool,
}

impl AuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the default group
    pub async fn get_default_group(&self) -> Result<Option<Group>, AppError> {
        sqlx::query_as!(
            Group,
            r#"
            SELECT id, name, description, permissions, is_system, is_active, is_default,
                   created_at as "created_at: _", updated_at as "updated_at: _"
            FROM groups
            WHERE is_default = true
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)
    }

    /// Assign user to default group
    pub async fn assign_user_to_default_group(&self, user_id: Uuid) -> Result<(), AppError> {
        let default_group = self.get_default_group().await?;

        if let Some(group) = default_group {
            sqlx::query!(
                r#"
                INSERT INTO user_groups (user_id, group_id, assigned_at)
                VALUES ($1, $2, NOW())
                "#,
                user_id,
                group.id
            )
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        }

        Ok(())
    }

    /// Find user auth link by provider and external ID
    pub async fn find_user_by_auth_link(
        &self,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let result = sqlx::query!(
            r#"
            SELECT user_id
            FROM user_auth_links
            WHERE provider_id = $1 AND external_id = $2
            "#,
            provider_id,
            external_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(result.map(|r| r.user_id))
    }

    /// Create a user auth link
    pub async fn create_auth_link(
        &self,
        user_id: Uuid,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO user_auth_links (user_id, provider_id, external_id, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
            user_id,
            provider_id,
            external_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// Create a new user from external auth (used by LDAP/OAuth)
    /// Returns the created user's ID
    pub async fn create_external_user(
        &self,
        username: &str,
        email: Option<String>,
        display_name: &str,
    ) -> Result<Uuid, AppError> {
        let new_user_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, display_name, is_active, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, true, false, NOW(), NOW())
            "#,
            new_user_id,
            username,
            email,
            display_name
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(new_user_id)
    }

    /// Create external user with auth link and assign to default group
    /// This is a convenience method that combines multiple operations
    pub async fn create_external_user_with_link(
        &self,
        username: &str,
        email: Option<String>,
        display_name: &str,
        provider_id: Uuid,
        external_id: &str,
    ) -> Result<Uuid, AppError> {
        let user_id = self
            .create_external_user(username, email, display_name)
            .await?;

        self.create_auth_link(user_id, provider_id, external_id)
            .await?;

        self.assign_user_to_default_group(user_id).await?;

        Ok(user_id)
    }
}
