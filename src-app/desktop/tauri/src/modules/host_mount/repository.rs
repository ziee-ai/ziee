//! DB layer for the host-mount feature (desktop crate, shared server pool).

use sqlx::PgPool;
use uuid::Uuid;
use ziee::AppError;

use super::models::{HostMountPolicyRow, MountEntry, UpdateHostMountPolicyRequest};

pub struct HostMountRepository {
    pool: PgPool,
}

impl HostMountRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ---- policy (singleton id=1) --------------------------------------

    pub async fn get_policy(&self) -> Result<HostMountPolicyRow, AppError> {
        let row = sqlx::query!(
            r#"SELECT enabled, allowed_prefixes, allow_readwrite
               FROM host_mount_policy WHERE id = 1"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(HostMountPolicyRow {
            enabled: row.enabled,
            allowed_prefixes: row.allowed_prefixes,
            allow_readwrite: row.allow_readwrite,
        })
    }

    pub async fn update_policy(
        &self,
        patch: &UpdateHostMountPolicyRequest,
    ) -> Result<HostMountPolicyRow, AppError> {
        let cur = self.get_policy().await?;
        let enabled = patch.enabled.unwrap_or(cur.enabled);
        let allowed = patch
            .allowed_prefixes
            .clone()
            .unwrap_or(cur.allowed_prefixes);
        let allow_rw = patch.allow_readwrite.unwrap_or(cur.allow_readwrite);

        let row = sqlx::query!(
            r#"UPDATE host_mount_policy
               SET enabled = $1, allowed_prefixes = $2, allow_readwrite = $3, updated_at = NOW()
               WHERE id = 1
               RETURNING enabled, allowed_prefixes, allow_readwrite"#,
            enabled,
            &allowed,
            allow_rw,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(HostMountPolicyRow {
            enabled: row.enabled,
            allowed_prefixes: row.allowed_prefixes,
            allow_readwrite: row.allow_readwrite,
        })
    }

    // ---- per-scope mount rows -----------------------------------------

    /// The conversation's own mount list, or `None` if it has no row (so the
    /// caller can fall through to the project).
    pub async fn conversation_mounts(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Vec<MountEntry>>, AppError> {
        let rec = sqlx::query!(
            r#"SELECT mounts as "mounts!: serde_json::Value"
               FROM host_mounts WHERE conversation_id = $1 AND user_id = $2"#,
            conversation_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rec.map(|r| serde_json::from_value(r.mounts).unwrap_or_default()))
    }

    pub async fn project_mounts(
        &self,
        project_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Vec<MountEntry>>, AppError> {
        let rec = sqlx::query!(
            r#"SELECT mounts as "mounts!: serde_json::Value"
               FROM host_mounts WHERE project_id = $1 AND user_id = $2"#,
            project_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rec.map(|r| serde_json::from_value(r.mounts).unwrap_or_default()))
    }

    pub async fn upsert_conversation(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
        mounts: &[MountEntry],
    ) -> Result<(), AppError> {
        let mounts_json = serde_json::to_value(mounts)
            .map_err(|e| AppError::internal_error(format!("serialize mounts: {e}")))?;
        sqlx::query!(
            r#"INSERT INTO host_mounts (conversation_id, user_id, mounts)
               VALUES ($1, $2, $3)
               ON CONFLICT (conversation_id) WHERE conversation_id IS NOT NULL
               DO UPDATE SET mounts = EXCLUDED.mounts, updated_at = NOW()"#,
            conversation_id,
            user_id,
            mounts_json,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    pub async fn upsert_project(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        mounts: &[MountEntry],
    ) -> Result<(), AppError> {
        let mounts_json = serde_json::to_value(mounts)
            .map_err(|e| AppError::internal_error(format!("serialize mounts: {e}")))?;
        sqlx::query!(
            r#"INSERT INTO host_mounts (project_id, user_id, mounts)
               VALUES ($1, $2, $3)
               ON CONFLICT (project_id) WHERE project_id IS NOT NULL
               DO UPDATE SET mounts = EXCLUDED.mounts, updated_at = NOW()"#,
            project_id,
            user_id,
            mounts_json,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    // ---- ownership + read-through resolution --------------------------

    pub async fn conversation_owner(&self, conversation_id: Uuid) -> Result<Option<Uuid>, AppError> {
        let r = sqlx::query!(
            r#"SELECT user_id FROM conversations WHERE id = $1"#,
            conversation_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(r.map(|r| r.user_id))
    }

    pub async fn project_owner(&self, project_id: Uuid) -> Result<Option<Uuid>, AppError> {
        let r = sqlx::query!(r#"SELECT user_id FROM projects WHERE id = $1"#, project_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        Ok(r.map(|r| r.user_id))
    }

    pub async fn conversation_project_id(
        &self,
        conversation_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let r = sqlx::query!(
            r#"SELECT project_id FROM project_conversations WHERE conversation_id = $1"#,
            conversation_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(r.map(|r| r.project_id))
    }

    /// Read-through resolution used by the sandbox provider: the conversation's
    /// own mounts if it has a row, else its project's mounts, else none.
    pub async fn resolve_effective(
        &self,
        conversation_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<MountEntry>, AppError> {
        if let Some(own) = self.conversation_mounts(conversation_id, user_id).await? {
            return Ok(own);
        }
        if let Some(project_id) = self.conversation_project_id(conversation_id).await? {
            if let Some(project_mounts) = self.project_mounts(project_id, user_id).await? {
                return Ok(project_mounts);
            }
        }
        Ok(Vec::new())
    }
}
