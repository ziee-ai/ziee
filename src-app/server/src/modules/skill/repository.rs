//! Minimum repository surface needed by the install handlers (B2).
//!
//! B6 fleshes out the full CRUD + the visibility-query union that backs
//! the chat extension + `skill_mcp::list_tools`.

#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{CreateSkill, Skill};
use crate::common::AppError;

pub struct SkillRepository {
    pool: PgPool,
}

impl SkillRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, request: CreateSkill) -> Result<Skill, AppError> {
        insert(&self.pool, request).await
    }

    pub async fn find_by_name_version(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<Skill>, AppError> {
        find_by_name_version(&self.pool, name, version).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        delete(&self.pool, id).await
    }
}

/// Insert one skill row. Returns the created row (with server-generated
/// id + timestamps). The scope/owner CHECK constraint
/// (`skills_scope_owner_check`) is enforced at the DB layer — caller
/// MUST set `owner_user_id` iff `scope == 'user'`.
pub async fn insert(pool: &PgPool, request: CreateSkill) -> Result<Skill, AppError> {
    let row = sqlx::query_as!(
        Skill,
        r#"
        INSERT INTO skills (
            name, version, display_name, description, when_to_use,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point, frontmatter_json, tags,
            scope, owner_user_id, created_by, enabled, is_dev
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9,
            $10, $11, $12,
            $13, $14, $15, $16, $17
        )
        RETURNING
            id,
            name,
            version,
            display_name,
            description,
            when_to_use,
            extracted_path,
            bundle_sha256,
            bundle_size_bytes,
            file_count,
            entry_point,
            frontmatter_json as "frontmatter_json: _",
            tags as "tags: _",
            scope,
            owner_user_id,
            created_by,
            enabled,
            is_dev,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        request.name,
        request.version,
        request.display_name,
        request.description,
        request.when_to_use,
        request.extracted_path,
        request.bundle_sha256,
        request.bundle_size_bytes,
        request.file_count,
        request.entry_point,
        request.frontmatter_json,
        request.tags,
        request.scope,
        request.owner_user_id,
        request.created_by,
        request.enabled,
        request.is_dev,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn find_by_name_version(
    pool: &PgPool,
    name: &str,
    version: Option<&str>,
) -> Result<Option<Skill>, AppError> {
    let row = sqlx::query_as!(
        Skill,
        r#"
        SELECT
            id,
            name,
            version,
            display_name,
            description,
            when_to_use,
            extracted_path,
            bundle_sha256,
            bundle_size_bytes,
            file_count,
            entry_point,
            frontmatter_json as "frontmatter_json: _",
            tags as "tags: _",
            scope,
            owner_user_id,
            created_by,
            enabled,
            is_dev,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM skills
        WHERE name = $1
          AND ($2::text IS NULL AND version IS NULL OR version = $2)
        LIMIT 1
        "#,
        name,
        version,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let n = sqlx::query!("DELETE FROM skills WHERE id = $1", id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Skill"));
    }
    Ok(())
}
