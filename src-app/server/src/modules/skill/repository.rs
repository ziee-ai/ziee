//! Repository surface needed by install handlers (B2) + chat extension +
//! skill_mcp + REST surface (B3).
//!
//! `list_available_for_conversation` is the visibility-query union backing
//! both the chat extension's listing-only injection AND
//! `skill_mcp::list_tools` — see plan §3 + §4.6 for the SQL.

#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{CreateSkill, Skill, UpdateSkill};
use crate::common::AppError;

pub struct SkillRepository {
    pool: PgPool,
}

/// Slim row used by both the chat extension (system-message listing) and
/// `skill_mcp::list_tools`. Heavy fields (`extracted_path`,
/// `frontmatter_json`, etc.) are deliberately omitted — only the
/// fields the LLM needs to decide whether to call `load_skill`.
#[derive(Debug, Clone)]
pub struct SkillAvailableEntry {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub when_to_use: Option<String>,
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

    /// H1: find the row for one (name, version) within a SPECIFIC owner
    /// scope. `owner_user_id = Some(uid)` matches that user's user-scope
    /// row; `None` matches the system-scope row. Used by the re-install
    /// overwrite path so a user re-installing the same hub skill replaces
    /// THEIR row (not another user's, not the system copy).
    pub async fn find_by_name_version_owner(
        &self,
        name: &str,
        version: Option<&str>,
        owner_user_id: Option<Uuid>,
    ) -> Result<Option<Skill>, AppError> {
        find_by_name_version_owner(&self.pool, name, version, owner_user_id).await
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Skill>, AppError> {
        find_by_id(&self.pool, id).await
    }

    /// Look up a skill by reverse-DNS name (any version). Used by
    /// `skill_mcp::load_skill` + `read_skill_file` to resolve a name
    /// the LLM passed.
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Skill>, AppError> {
        find_by_name(&self.pool, name).await
    }

    /// M5: resolve a skill by name to the row the CALLER can actually read,
    /// preferring the user's own copy over an accessible system copy. Used by
    /// `skill_mcp` so a same-named skill owned by another user can't shadow
    /// (and make uncallable) the caller's own installed skill.
    pub async fn find_accessible_by_name(
        &self,
        user_id: Uuid,
        name: &str,
    ) -> Result<Option<Skill>, AppError> {
        find_accessible_by_name(&self.pool, user_id, name).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        delete(&self.pool, id).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        request: UpdateSkill,
    ) -> Result<Skill, AppError> {
        update(&self.pool, id, request).await
    }

    /// List the skills available to `user_id` in `conversation_id` —
    /// the union of (user-owned) + (system, not group-restricted OR in
    /// the user's groups), minus per-conversation hides.
    ///
    /// This is the single source of truth for the chat extension's
    /// listing AND for `skill_mcp::list_tools`. See plan §3 SQL spec.
    pub async fn list_available_for_conversation(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<SkillAvailableEntry>, AppError> {
        list_available_for_conversation(&self.pool, user_id, conversation_id).await
    }

    /// Variant for tool-call paths that don't have a conversation in
    /// hand (e.g. a future skill_mcp tool call with no
    /// `x-conversation-id`). Same union minus the per-conversation
    /// hide filter.
    pub async fn list_accessible(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Skill>, AppError> {
        list_accessible(&self.pool, user_id).await
    }

    /// List all system-scope skills (admin surface).
    pub async fn list_system(&self) -> Result<Vec<Skill>, AppError> {
        list_system(&self.pool).await
    }

    /// Is this skill currently hidden in this conversation?
    pub async fn is_hidden_in_conversation(
        &self,
        skill_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<bool, AppError> {
        is_hidden_in_conversation(&self.pool, skill_id, conversation_id).await
    }

    pub async fn set_hidden_in_conversation(
        &self,
        skill_id: Uuid,
        conversation_id: Uuid,
        hidden: bool,
    ) -> Result<(), AppError> {
        set_hidden_in_conversation(&self.pool, skill_id, conversation_id, hidden).await
    }

    pub async fn clear_hidden_in_conversation(
        &self,
        skill_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<(), AppError> {
        clear_hidden_in_conversation(&self.pool, skill_id, conversation_id).await
    }

    /// Check accessibility: a user can read a skill iff they own it
    /// (user-scope) OR it's a system skill without group restrictions
    /// OR it's a system skill assigned to one of their groups.
    pub async fn user_can_read(
        &self,
        user_id: Uuid,
        skill_id: Uuid,
    ) -> Result<bool, AppError> {
        user_can_read(&self.pool, user_id, skill_id).await
    }

    /// Group assignment management for system-scope skills.
    pub async fn get_skill_groups(&self, skill_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        get_skill_groups(&self.pool, skill_id).await
    }

    pub async fn assign_skill_to_group(
        &self,
        skill_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), AppError> {
        assign_skill_to_group(&self.pool, skill_id, group_id).await
    }

    pub async fn remove_skill_from_group(
        &self,
        skill_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), AppError> {
        remove_skill_from_group(&self.pool, skill_id, group_id).await
    }

    /// On-disk `extracted_path`s of a user's own (scope='user') skills.
    /// Used by the user-delete path to rm the bundle dirs before the rows
    /// cascade away (the FK is `ON DELETE CASCADE`, so this MUST be read
    /// before the user row is deleted).
    pub async fn list_owned_extracted_paths(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query!(
            "SELECT extracted_path FROM skills WHERE scope = 'user' AND owner_user_id = $1",
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| r.extracted_path).collect())
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

/// Upsert a built-in (scope='built_in') skill keyed on `name`. Used by the
/// boot sync so a binary upgrade replaces the row in place (stable id,
/// version-locked content). Conflict target is the `uniq_skills_builtin_name`
/// partial index.
pub async fn upsert_builtin(pool: &PgPool, request: CreateSkill) -> Result<Skill, AppError> {
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
            'built_in', NULL, NULL, TRUE, FALSE
        )
        ON CONFLICT (name) WHERE scope = 'built_in'
        DO UPDATE SET
            version = EXCLUDED.version,
            display_name = EXCLUDED.display_name,
            description = EXCLUDED.description,
            when_to_use = EXCLUDED.when_to_use,
            extracted_path = EXCLUDED.extracted_path,
            bundle_sha256 = EXCLUDED.bundle_sha256,
            bundle_size_bytes = EXCLUDED.bundle_size_bytes,
            file_count = EXCLUDED.file_count,
            entry_point = EXCLUDED.entry_point,
            frontmatter_json = EXCLUDED.frontmatter_json,
            tags = EXCLUDED.tags,
            enabled = TRUE,
            updated_at = NOW()
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
          AND (($2::text IS NULL AND version IS NULL) OR version = $2)
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

/// H1: owner-scoped (name, version) lookup. NULL `owner_user_id` matches
/// the system row; a non-NULL value matches that user's row only.
pub async fn find_by_name_version_owner(
    pool: &PgPool,
    name: &str,
    version: Option<&str>,
    owner_user_id: Option<Uuid>,
) -> Result<Option<Skill>, AppError> {
    let row = sqlx::query_as!(
        Skill,
        r#"
        SELECT
            id, name, version, display_name, description, when_to_use,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point,
            frontmatter_json as "frontmatter_json: _",
            tags as "tags: _", scope, owner_user_id, created_by,
            enabled, is_dev,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM skills
        WHERE name = $1
          AND (($2::text IS NULL AND version IS NULL) OR version = $2)
          AND owner_user_id IS NOT DISTINCT FROM $3
        LIMIT 1
        "#,
        name,
        version,
        owner_user_id,
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

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Skill>, AppError> {
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
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Skill>, AppError> {
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
        ORDER BY version DESC NULLS LAST
        LIMIT 1
        "#,
        name,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// M5: name → the row the caller can read, preferring the user's own copy.
/// Same access predicate as `user_can_read` (user-owned OR accessible
/// system), ordered so the user's own copy wins over a system copy, then by
/// version. This makes a same-named cross-user install unable to shadow the
/// caller's own skill (the old `find_by_name` picked the global highest
/// version then access-checked it, so another user's higher-versioned copy
/// made the caller's own skill resolve to a forbidden row).
pub async fn find_accessible_by_name(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
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
        FROM skills s
        WHERE s.name = $1
          -- M-1: a disabled skill must not be loadable by the LLM via
          -- skill_mcp (it's already excluded from the chat listing). The
          -- owner can still read/manage it via the REST path, which uses
          -- user_can_read (deliberately without this filter).
          AND s.enabled = TRUE
          AND (
            s.scope = 'built_in'
            OR (s.scope = 'user' AND s.owner_user_id = $2)
            OR (s.scope = 'system' AND (
              NOT EXISTS (SELECT 1 FROM group_skills WHERE skill_id = s.id)
              OR EXISTS (
                SELECT 1 FROM group_skills gs
                JOIN user_groups ug ON gs.group_id = ug.group_id
                WHERE gs.skill_id = s.id AND ug.user_id = $2
              )
            ))
          )
        ORDER BY (s.scope = 'user') DESC, s.version DESC NULLS LAST
        LIMIT 1
        "#,
        name,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    request: UpdateSkill,
) -> Result<Skill, AppError> {
    let row = sqlx::query_as!(
        Skill,
        r#"
        UPDATE skills SET
            display_name = COALESCE($2, display_name),
            description = COALESCE($3, description),
            when_to_use = COALESCE($4, when_to_use),
            enabled = COALESCE($5, enabled),
            tags = COALESCE($6, tags),
            updated_at = NOW()
        WHERE id = $1
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
        id,
        request.display_name,
        request.description,
        request.when_to_use,
        request.enabled,
        request.tags,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Skill"))?;
    Ok(row)
}

/// Available-skills view per plan §3:
/// User-owned (scope='user', owner=user_id) ∪ system (no group restriction
/// OR user is in an assigned group), minus per-conversation hides.
/// `enabled=TRUE` only. Returns just the lightweight fields needed for
/// the chat-extension listing + `skill_mcp::list_tools`.
pub async fn list_available_for_conversation(
    pool: &PgPool,
    user_id: Uuid,
    conversation_id: Uuid,
) -> Result<Vec<SkillAvailableEntry>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id as "id!", s.name as "name!", s.description, s.when_to_use
        FROM skills s
        WHERE s.enabled = TRUE
          AND (
            -- Built-in capability skills: always available to everyone.
            s.scope = 'built_in'
            OR (s.scope = 'user' AND s.owner_user_id = $1)
            OR
            (s.scope = 'system' AND (
              NOT EXISTS (SELECT 1 FROM group_skills WHERE skill_id = s.id)
              OR EXISTS (
                SELECT 1 FROM group_skills gs
                JOIN user_groups ug ON gs.group_id = ug.group_id
                WHERE gs.skill_id = s.id AND ug.user_id = $1
              )
            ))
          )
          AND NOT EXISTS (
            SELECT 1 FROM conversation_skill_overrides
            WHERE skill_id = s.id AND conversation_id = $2 AND hidden = TRUE
          )
        ORDER BY s.name ASC
        "#,
        user_id,
        conversation_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows
        .into_iter()
        .map(|r| SkillAvailableEntry {
            id: r.id,
            name: r.name,
            description: r.description,
            when_to_use: r.when_to_use,
        })
        .collect())
}

/// Accessible-skills (no conversation context). Same union sans the
/// conversation-hide filter; returns full rows so the REST `/api/skills`
/// list endpoint + skill_mcp's no-conversation path can use it.
pub async fn list_accessible(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Skill>, AppError> {
    let rows = sqlx::query_as!(
        Skill,
        r#"
        SELECT
            s.id, s.name, s.version, s.display_name, s.description,
            s.when_to_use, s.extracted_path, s.bundle_sha256,
            s.bundle_size_bytes, s.file_count, s.entry_point,
            s.frontmatter_json as "frontmatter_json: _",
            s.tags as "tags: _", s.scope, s.owner_user_id, s.created_by,
            s.enabled, s.is_dev,
            s.created_at as "created_at: _",
            s.updated_at as "updated_at: _"
        FROM skills s
        WHERE
            s.scope = 'built_in'
            OR (s.scope = 'user' AND s.owner_user_id = $1)
            OR (s.scope = 'system' AND (
                NOT EXISTS (SELECT 1 FROM group_skills WHERE skill_id = s.id)
                OR EXISTS (
                  SELECT 1 FROM group_skills gs
                  JOIN user_groups ug ON gs.group_id = ug.group_id
                  WHERE gs.skill_id = s.id AND ug.user_id = $1
                )
            ))
        ORDER BY s.name ASC
        "#,
        user_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

pub async fn list_system(pool: &PgPool) -> Result<Vec<Skill>, AppError> {
    let rows = sqlx::query_as!(
        Skill,
        r#"
        SELECT
            id, name, version, display_name, description, when_to_use,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point,
            frontmatter_json as "frontmatter_json: _",
            tags as "tags: _", scope, owner_user_id, created_by,
            enabled, is_dev,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM skills
        WHERE scope = 'system'
        ORDER BY name ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

pub async fn is_hidden_in_conversation(
    pool: &PgPool,
    skill_id: Uuid,
    conversation_id: Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query_scalar!(
        r#"
        SELECT hidden
        FROM conversation_skill_overrides
        WHERE skill_id = $1 AND conversation_id = $2
        "#,
        skill_id,
        conversation_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.unwrap_or(false))
}

pub async fn set_hidden_in_conversation(
    pool: &PgPool,
    skill_id: Uuid,
    conversation_id: Uuid,
    hidden: bool,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO conversation_skill_overrides (conversation_id, skill_id, hidden)
        VALUES ($1, $2, $3)
        ON CONFLICT (conversation_id, skill_id) DO UPDATE SET hidden = EXCLUDED.hidden
        "#,
        conversation_id,
        skill_id,
        hidden,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn clear_hidden_in_conversation(
    pool: &PgPool,
    skill_id: Uuid,
    conversation_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        DELETE FROM conversation_skill_overrides
        WHERE skill_id = $1 AND conversation_id = $2
        "#,
        skill_id,
        conversation_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Same union as `list_accessible` but bool-evaluated for a single
/// skill — used as the access-check shared by skill_mcp + REST get/edit/delete.
pub async fn user_can_read(
    pool: &PgPool,
    user_id: Uuid,
    skill_id: Uuid,
) -> Result<bool, AppError> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM skills s
        WHERE s.id = $1
          AND (
            s.scope = 'built_in'
            OR (s.scope = 'user' AND s.owner_user_id = $2)
            OR (s.scope = 'system' AND (
              NOT EXISTS (SELECT 1 FROM group_skills WHERE skill_id = s.id)
              OR EXISTS (
                SELECT 1 FROM group_skills gs
                JOIN user_groups ug ON gs.group_id = ug.group_id
                WHERE gs.skill_id = s.id AND ug.user_id = $2
              )
            ))
          )
        "#,
        skill_id,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(count > 0)
}

pub async fn get_skill_groups(pool: &PgPool, skill_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let rows = sqlx::query_scalar!(
        r#"SELECT group_id FROM group_skills WHERE skill_id = $1"#,
        skill_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

pub async fn assign_skill_to_group(
    pool: &PgPool,
    skill_id: Uuid,
    group_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO group_skills (group_id, skill_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
        group_id,
        skill_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn remove_skill_from_group(
    pool: &PgPool,
    skill_id: Uuid,
    group_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        DELETE FROM group_skills WHERE skill_id = $1 AND group_id = $2
        "#,
        skill_id,
        group_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}
