use super::models::*;
use sqlx::PgPool;
use uuid::Uuid;

pub struct UserGroupRepository {
    pool: PgPool,
}

impl UserGroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get group by ID
    pub async fn get_by_id(&self, group_id: Uuid) -> Result<Option<UserGroup>, sqlx::Error> {
        sqlx::query_as!(
            UserGroup,
            r#"
            SELECT
                id,
                name,
                description,
                permissions,
                is_protected,
                is_active,
                created_at,
                updated_at
            FROM user_groups
            WHERE id = $1
            "#,
            group_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get group by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<UserGroup>, sqlx::Error> {
        sqlx::query_as!(
            UserGroup,
            r#"
            SELECT
                id,
                name,
                description,
                permissions,
                is_protected,
                is_active,
                created_at,
                updated_at
            FROM user_groups
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Create a new user group
    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        permissions: Vec<String>,
    ) -> Result<UserGroup, sqlx::Error> {
        let permissions_json = serde_json::to_value(permissions)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        sqlx::query_as!(
            UserGroup,
            r#"
            INSERT INTO user_groups (name, description, permissions, is_protected, is_active)
            VALUES ($1, $2, $3, false, true)
            RETURNING id, name, description, permissions, is_protected, is_active, created_at, updated_at
            "#,
            name,
            description,
            permissions_json
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Update user group
    pub async fn update(
        &self,
        group_id: Uuid,
        name: Option<&str>,
        description: Option<Option<&str>>,
        permissions: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> Result<Option<UserGroup>, sqlx::Error> {
        // Get current group
        let mut group = match self.get_by_id(group_id).await? {
            Some(g) => g,
            None => return Ok(None),
        };

        // Update fields
        if let Some(n) = name {
            group.name = n.to_string();
        }
        if let Some(d) = description {
            group.description = d.map(|s| s.to_string());
        }
        if let Some(p) = permissions {
            group.set_permissions(p);
        }
        if let Some(a) = is_active {
            group.is_active = a;
        }

        // Save to database
        sqlx::query_as!(
            UserGroup,
            r#"
            UPDATE user_groups
            SET name = $2, description = $3, permissions = $4, is_active = $5, updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, permissions, is_protected, is_active, created_at, updated_at
            "#,
            group_id,
            group.name,
            group.description,
            group.permissions,
            group.is_active
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Delete user group
    pub async fn delete(&self, group_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_groups WHERE id = $1
            "#,
            group_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// List user groups with pagination
    pub async fn list(
        &self,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<UserGroup>, i64), sqlx::Error> {
        let offset = (page - 1) * per_page;

        // Get total count
        let total = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!" FROM user_groups
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        // Get paginated groups
        let groups = sqlx::query_as!(
            UserGroup,
            r#"
            SELECT
                id,
                name,
                description,
                permissions,
                is_protected,
                is_active,
                created_at,
                updated_at
            FROM user_groups
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            per_page as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await?;

        Ok((groups, total))
    }

    /// Assign user to group
    pub async fn assign_user(
        &self,
        user_id: Uuid,
        group_id: Uuid,
        assigned_by: Option<Uuid>,
    ) -> Result<UserGroupMembership, sqlx::Error> {
        sqlx::query_as!(
            UserGroupMembership,
            r#"
            INSERT INTO user_group_memberships (user_id, group_id, assigned_by)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, group_id) DO UPDATE
            SET assigned_at = NOW(), assigned_by = $3
            RETURNING id, user_id, group_id, assigned_at, assigned_by
            "#,
            user_id,
            group_id,
            assigned_by
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Remove user from group
    pub async fn remove_user(
        &self,
        user_id: Uuid,
        group_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM user_group_memberships WHERE user_id = $1 AND group_id = $2
            "#,
            user_id,
            group_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all members of a group
    pub async fn get_group_members(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<UserGroupMembershipWithUser>, sqlx::Error> {
        sqlx::query_as!(
            UserGroupMembershipWithUser,
            r#"
            SELECT
                m.id,
                m.user_id,
                m.group_id,
                m.assigned_at,
                m.assigned_by,
                u.username
            FROM user_group_memberships m
            JOIN users u ON m.user_id = u.id
            WHERE m.group_id = $1
            ORDER BY m.assigned_at DESC
            "#,
            group_id
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get all groups for a user
    pub async fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<UserGroup>, sqlx::Error> {
        sqlx::query_as!(
            UserGroup,
            r#"
            SELECT
                g.id,
                g.name,
                g.description,
                g.permissions,
                g.is_protected,
                g.is_active,
                g.created_at,
                g.updated_at
            FROM user_groups g
            JOIN user_group_memberships m ON g.id = m.group_id
            WHERE m.user_id = $1 AND g.is_active = true
            ORDER BY g.name
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
    }
}
