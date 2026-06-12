//! REST handlers for `/api/memories` and `/api/memory/admin-settings`.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        memory::{
            models::{
                CreateMemoryRequest, MemoryAdminSettings, MemoryAuditEntry,
                MemoryListResponse, UpdateMemoryAdminSettingsRequest,
                UpdateMemoryRequest, UpdateUserMemorySettingsRequest, UserMemory,
                UserMemorySettings, is_valid_kind,
            },
            permissions::{MemoryAdminManage, MemoryAdminRead, MemoryRead, MemoryWrite},
        },
        permissions::{RequirePermissions, with_permission},
        sync::{
            Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish,
        },
    },
};

// Hard cap on memory content length — defends against pathological writes.
// Shared with memory_mcp/handlers.rs (audit R7-#8 dedup).
use crate::modules::memory::models::MAX_MEMORY_CONTENT_LEN as MAX_CONTENT_LEN;

/// List/page query params. Accepts `page` + `per_page` (1-based)
/// plus optional server-side filters that get pushed all the way
/// down to the SQL WHERE clause:
///
///   - `search` — case-insensitive substring match on `content`
///   - `kind`   — exact match on the memory kind
///                (preference / fact / goal / relationship / other)
///   - `source` — exact match on the source
///                (manual / extraction / mcp_tool)
///
/// `limit`/`offset` are still accepted as legacy fallback for
/// scripted callers — the standard page/per_page path is preferred.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListMemoriesQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
    /// Legacy: when `page`/`per_page` are absent, fall back to
    /// `limit`/`offset` (clamped to safe ranges).
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
    /// Substring search on `content` (case-insensitive). Trimmed +
    /// empty-string normalized to None.
    #[serde(default)]
    pub search: Option<String>,
    /// Exact-match filter on `kind`. None = no filter.
    #[serde(default)]
    pub kind: Option<String>,
    /// Exact-match filter on `source`. None = no filter.
    #[serde(default)]
    pub source: Option<String>,
}

#[debug_handler]
pub async fn list_memories(
    auth: RequirePermissions<(MemoryRead,)>,
    Query(q): Query<ListMemoriesQuery>,
) -> ApiResult<Json<MemoryListResponse>> {
    // Resolve page/per_page from either the new shape or the
    // legacy limit/offset, then clamp to a defensive range.
    let per_page = q
        .per_page
        .or(q.limit)
        .unwrap_or(50)
        .clamp(1, 200);
    let page = if let Some(p) = q.page {
        p.max(1)
    } else if let Some(off) = q.offset {
        // Convert offset → 1-based page using per_page as the divisor.
        (off.max(0) / per_page) + 1
    } else {
        1
    };
    let offset = (page - 1) * per_page;

    // Normalize: trim search, treat empty as None so the SQL noop
    // short-circuits and we don't run `ILIKE '%%'`.
    let search = q
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let kind = q
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let source = q
        .source
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let items = Repos
        .memory
        .list_for_user(auth.user.id, per_page, offset, search, kind, source)
        .await?;
    let total = Repos
        .memory
        .count_for_user(auth.user.id, search, kind, source)
        .await?;

    Ok((
        StatusCode::OK,
        Json(MemoryListResponse {
            items,
            total,
            page,
            per_page,
        }),
    ))
}

pub fn list_memories_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryRead,)>(op)
        .id("Memory.list")
        .tag("Memory")
        .summary("List the caller's own memories (paginated)")
        .response::<200, Json<MemoryListResponse>>()
}

#[debug_handler]
pub async fn get_memory(
    auth: RequirePermissions<(MemoryRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<UserMemory>> {
    let row = Repos
        .memory
        .get_owned(auth.user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Memory"))?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_memory_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryRead,)>(op)
        .id("Memory.get")
        .tag("Memory")
        .summary("Fetch a single owned memory")
        .response::<200, Json<UserMemory>>()
        .response_with::<404, (), _>(|r| r.description("Not found or not owned"))
}

#[debug_handler]
pub async fn create_memory(
    auth: RequirePermissions<(MemoryWrite,)>,
    origin: SyncOrigin,
    Json(body): Json<CreateMemoryRequest>,
) -> ApiResult<Json<UserMemory>> {
    let content = body.content.trim();
    if content.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "content must not be empty").into());
    }
    if content.chars().count() > MAX_CONTENT_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "content exceeds 4000 char limit",
        )
        .into());
    }
    if !is_valid_kind(&body.kind) {
        return Err(AppError::bad_request("VALIDATION_ERROR", "invalid kind").into());
    }
    if !(0..=100).contains(&body.importance) {
        return Err(AppError::bad_request("VALIDATION_ERROR", "importance must be 0..=100").into());
    }
    // Enforce per-user cap; Phase 5 wires `user_memory_settings.max_memories`.
    let row = Repos
        .memory
        .insert(
            auth.user.id,
            content,
            "manual",
            body.importance,
            &body.kind,
            &body.metadata,
            None,
            // Manual REST adds are user-global.
            "user",
            None,
            None,
        )
        .await?;
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Create,
        row.id,
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::CREATED, Json(row)))
}

pub fn create_memory_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryWrite,)>(op)
        .id("Memory.create")
        .tag("Memory")
        .summary("Manually create a memory")
        .response::<201, Json<UserMemory>>()
        .response_with::<400, (), _>(|r| r.description("Validation error"))
}

#[debug_handler]
pub async fn update_memory(
    auth: RequirePermissions<(MemoryWrite,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(body): Json<UpdateMemoryRequest>,
) -> ApiResult<Json<UserMemory>> {
    if let Some(c) = &body.content {
        if c.trim().is_empty() {
            return Err(
                AppError::bad_request("VALIDATION_ERROR", "content must not be empty").into(),
            );
        }
        if c.chars().count() > MAX_CONTENT_LEN {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "content exceeds 4000 char limit",
            )
            .into());
        }
    }
    if let Some(k) = &body.kind {
        if !is_valid_kind(k) {
            return Err(AppError::bad_request("VALIDATION_ERROR", "invalid kind").into());
        }
    }
    if let Some(i) = body.importance {
        if !(0..=100).contains(&i) {
            return Err(
                AppError::bad_request("VALIDATION_ERROR", "importance must be 0..=100").into(),
            );
        }
    }
    let row = Repos
        .memory
        .update_owned(
            auth.user.id,
            id,
            body.content.as_deref(),
            body.importance,
            body.kind.as_deref(),
            body.metadata.as_ref(),
        )
        .await?
        .ok_or_else(|| AppError::not_found("Memory"))?;
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Update,
        row.id,
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_memory_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryWrite,)>(op)
        .id("Memory.update")
        .tag("Memory")
        .summary("Edit an owned memory")
        .response::<200, Json<UserMemory>>()
        .response_with::<404, (), _>(|r| r.description("Not found or not owned"))
}

#[debug_handler]
pub async fn delete_memory(
    auth: RequirePermissions<(MemoryWrite,)>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    let deleted = Repos.memory.soft_delete_owned(auth.user.id, id).await?;
    if !deleted {
        return Err(AppError::not_found("Memory").into());
    }
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Delete,
        id,
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_memory_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryWrite,)>(op)
        .id("Memory.delete")
        .tag("Memory")
        .summary("Delete an owned memory (soft delete)")
        .response_with::<204, (), _>(|r| r.description("Deleted"))
        .response_with::<404, (), _>(|r| r.description("Not found or not owned"))
}

#[debug_handler]
pub async fn delete_all_memories(
    auth: RequirePermissions<(MemoryWrite,)>,
    origin: SyncOrigin,
) -> ApiResult<Json<DeleteAllResponse>> {
    let n = Repos.memory.hard_delete_all_for_user(auth.user.id).await?;
    // No single entity id for a bulk clear; the client's memory handler
    // reloads the list regardless of id (nil acts as "everything changed").
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Delete,
        Uuid::nil(),
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((
        StatusCode::OK,
        Json(DeleteAllResponse { deleted: n as i64 }),
    ))
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct DeleteAllResponse {
    pub deleted: i64,
}

pub fn delete_all_memories_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryWrite,)>(op)
        .id("Memory.deleteAll")
        .tag("Memory")
        .summary("Hard-delete every memory for the caller")
        .response::<200, Json<DeleteAllResponse>>()
}

// ── audit log ───────────────────────────────────────────────────────

/// Optional `?limit=N` query param for the audit-log endpoint.
/// Clamp 1..=500 happens in the repo. Audit R7-#2.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAuditLogQuery {
    #[serde(default = "default_audit_limit")]
    pub limit: i64,
}
fn default_audit_limit() -> i64 {
    100
}

#[debug_handler]
pub async fn list_audit_log(
    auth: RequirePermissions<(MemoryRead,)>,
    Query(q): Query<ListAuditLogQuery>,
) -> ApiResult<Json<Vec<MemoryAuditEntry>>> {
    let rows = Repos.memory.list_audit_log(auth.user.id, q.limit).await?;
    Ok((StatusCode::OK, Json(rows)))
}

pub fn list_audit_log_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryRead,)>(op)
        .id("MemoryAudit.list")
        .tag("Memory")
        .summary("List the caller's memory audit log entries")
        .response::<200, Json<Vec<MemoryAuditEntry>>>()
}

// ── user memory settings ────────────────────────────────────────────

#[debug_handler]
pub async fn get_user_settings(
    auth: RequirePermissions<(MemoryRead,)>,
) -> ApiResult<Json<UserMemorySettings>> {
    let row = Repos.memory.get_or_init_user_settings(auth.user.id).await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_user_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryRead,)>(op)
        .id("MemorySettings.get")
        .tag("Memory")
        .summary("Fetch the caller's memory settings")
        .response::<200, Json<UserMemorySettings>>()
}

#[debug_handler]
pub async fn update_user_settings(
    auth: RequirePermissions<(MemoryWrite,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateUserMemorySettingsRequest>,
) -> ApiResult<Json<UserMemorySettings>> {
    if let Some(n) = body.max_memories {
        if !(1..=100_000).contains(&n) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "max_memories out of range",
            )
            .into());
        }
    }
    if let Some(Some(d)) = body.retention_days {
        if !(1..=3_650).contains(&d) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "retention_days out of range (1..=3650)",
            )
            .into());
        }
    }
    let row = Repos
        .memory
        .update_user_settings(
            auth.user.id,
            body.extraction_enabled,
            body.retrieval_enabled,
            body.max_memories,
            body.retention_days,
            body.extraction_model_id,
        )
        .await?;
    sync_publish(
        SyncEntity::MemorySettings,
        SyncAction::Update,
        auth.user.id,
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_user_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryWrite,)>(op)
        .id("MemorySettings.update")
        .tag("Memory")
        .summary("Update the caller's memory settings")
        .response::<200, Json<UserMemorySettings>>()
}

// ── admin settings ──────────────────────────────────────────────────

#[debug_handler]
pub async fn get_admin_settings(
    _auth: RequirePermissions<(MemoryAdminRead,)>,
) -> ApiResult<Json<MemoryAdminSettings>> {
    let row = Repos.memory.get_admin_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminRead,)>(op)
        .id("MemoryAdmin.get")
        .tag("Memory")
        .summary("Read admin memory settings")
        .response::<200, Json<MemoryAdminSettings>>()
}

#[debug_handler]
pub async fn update_admin_settings(
    _auth: RequirePermissions<(MemoryAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateMemoryAdminSettingsRequest>,
) -> ApiResult<Json<MemoryAdminSettings>> {
    if let Some(k) = body.default_top_k {
        if !(1..=100).contains(&k) {
            return Err(
                AppError::bad_request("VALIDATION_ERROR", "default_top_k out of range").into(),
            );
        }
    }
    if let Some(t) = body.cosine_threshold {
        if !(0.0..=2.0).contains(&t) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "cosine_threshold out of range (0.0..=2.0)",
            )
            .into());
        }
    }
    // Summarizer-field validation lived here pre-migration-91; the
    // four fields moved to the `summarization` module along with the
    // engine + per-conversation toggle.

    // FTS validation (migration 89). Range bounds mirror the CHECK
    // constraints — handler returns 400 with a clean reason instead of
    // a raw 500 from the DB.
    if let Some(k) = body.fts_rrf_k {
        if !(1..=1000).contains(&k) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "fts_rrf_k out of range (1..=1000)",
            )
            .into());
        }
    }
    if let Some(m) = body.fts_candidate_multiplier {
        if !(1..=20).contains(&m) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "fts_candidate_multiplier out of range (1..=20)",
            )
            .into());
        }
    }
    if let Some(r) = body.fts_min_rank {
        if !(0.0..=1.0).contains(&r) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "fts_min_rank out of range (0.0..=1.0)",
            )
            .into());
        }
    }
    if let Some(ref d) = body.fts_dictionary {
        if !super::models::is_valid_fts_dictionary(d) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "fts_dictionary not in allowlist",
            )
            .into());
        }
    }

    // Snapshot the current embedding model id so we can detect a swap
    // and trigger the re-embed worker if it changed.
    let prior = Repos.memory.get_admin_settings().await?;
    let prior_model_id = prior.embedding_model_id;

    // Dictionary changes can't go through the regular PUT path — the
    // GENERATED expression on `user_memories.content_tsv` bakes the
    // dictionary in at column-creation time and can't be ALTERed in
    // place. Force the caller to hit POST /memory/admin/fts/rebuild
    // (which atomically swaps the dictionary + rebuilds the column).
    if let Some(ref d) = body.fts_dictionary {
        if d != &prior.fts_dictionary {
            return Err(AppError::new(
                axum::http::StatusCode::CONFLICT,
                "FTS_REBUILD_REQUIRED",
                "fts_dictionary changes must go through POST /api/memory/admin/fts/rebuild",
            )
            .into());
        }
    }

    let row = Repos
        .memory
        .update_admin_settings(
            body.embedding_model_id.clone(),
            body.default_extraction_model_id,
            body.default_top_k,
            body.cosine_threshold,
            body.enabled,
            body.soft_delete_grace_days,
            body.daily_extraction_quota,
            body.fts_enabled,
            body.fts_rrf_k,
            body.fts_candidate_multiplier,
            body.fts_min_rank,
            body.semantic_enabled,
        )
        .await?;

    // If the admin swapped the embedding model (or set one for the
    // first time after onboarding), kick off the worker. The worker
    // handles both same-dim re-embed and cross-dim ALTER COLUMN +
    // re-embed. Fire-and-forget so the HTTP call returns immediately.
    if let Some(new_id_opt) = body.embedding_model_id {
        if new_id_opt != prior_model_id {
            if let Some(new_id) = new_id_opt {
                // Probe the new model to learn its name + dimension.
                // The model row may not exist in extreme races; fall
                // back to no-op if so.
                if let Ok(Some(new_model)) = Repos.llm_model.get_by_id(new_id).await {
                    // Best-effort dimension probe via a single embed
                    // of a sentinel string. Allows recording the
                    // actual model dimension into memory_admin_settings
                    // and gates the cross-dim ALTER path.
                    let pool = Repos.memory.pool_clone();
                    let model_id = new_id;
                    let model_name = new_model.name.clone();
                    tokio::spawn(async move {
                        let dim = match crate::modules::memory::engine::dispatch::embed(
                            model_id,
                            "dimension probe",
                        )
                        .await
                        {
                            Ok(v) => v.len() as i32,
                            Err(e) => {
                                tracing::warn!(
                                    "memory.admin: dimension probe failed for {}: {} — keeping existing dim",
                                    model_id,
                                    e
                                );
                                return;
                            }
                        };
                        super::embedding_worker::reembed_all(pool, model_id, model_name, dim).await;
                    });
                }
            }
        }
    }

    sync_publish(
        SyncEntity::MemoryAdminSettings,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<MemoryAdminRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(row)))
}

pub fn update_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminManage,)>(op)
        .id("MemoryAdmin.update")
        .tag("Memory")
        .summary("Update admin memory settings")
        .response::<200, Json<MemoryAdminSettings>>()
}

// ============================================================================
// Rebuild status + explicit trigger.
//
// Two endpoints supporting the admin UX around the embedding-model
// rebuild worker:
//
//   GET  /memory/admin-settings/rebuild-status
//       → { in_progress, pending_count, model_name }
//   POST /memory/admin-settings/reembed
//       → 202 — spawn reembed_all for the CURRENT admin.embedding_model_id
//
// `trigger_reembed` was added because the auto-trigger in
// `update_admin_settings` only fires when `embedding_model_id`
// CHANGES — so "Re-embed now" re-PUTing the same id was silently
// a no-op. This explicit endpoint fixes that, and doubles as the
// test hook for verifying resume-after-stale-rows behavior.
// ============================================================================

#[derive(Debug, serde::Serialize, Deserialize, JsonSchema)]
pub struct RebuildStatus {
    /// True while a rebuild worker holds the process-local lock.
    pub in_progress: bool,
    /// Live rows still needing (re)embedding under the current model:
    /// embedding IS NULL OR embedding_model != current_model.name.
    /// Returns 0 if no embedding model is configured.
    pub pending_count: i64,
    /// Name of the currently-configured embedding model, if any.
    pub model_name: Option<String>,
}

#[debug_handler]
pub async fn get_rebuild_status(
    _auth: RequirePermissions<(MemoryAdminRead,)>,
) -> ApiResult<Json<RebuildStatus>> {
    let in_progress = super::embedding_worker::is_in_progress();
    let admin = Repos.memory.get_admin_settings().await?;
    let (pending_count, model_name) = if let Some(model_id) = admin.embedding_model_id {
        let model = Repos
            .llm_model
            .get_by_id(model_id)
            .await
            .map_err(AppError::database_error)?
            .ok_or_else(|| AppError::not_found("LlmModel"))?;
        let pool = Repos.memory.pool_clone();
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM user_memories
               WHERE deleted_at IS NULL
                 AND (embedding IS NULL OR embedding_model IS DISTINCT FROM $1)"#,
            &model.name
        )
        .fetch_one(&pool)
        .await
        .map_err(AppError::database_error)?;
        (n, Some(model.name))
    } else {
        (0, None)
    };
    Ok((
        StatusCode::OK,
        Json(RebuildStatus {
            in_progress,
            pending_count,
            model_name,
        }),
    ))
}

pub fn get_rebuild_status_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminRead,)>(op)
        .id("MemoryAdmin.rebuildStatus")
        .tag("Memory")
        .summary("Read embedding rebuild status")
        .response::<200, Json<RebuildStatus>>()
}

#[debug_handler]
pub async fn trigger_reembed(
    _auth: RequirePermissions<(MemoryAdminManage,)>,
) -> ApiResult<Json<serde_json::Value>> {
    let admin = Repos.memory.get_admin_settings().await?;
    let Some(model_id) = admin.embedding_model_id else {
        return Err(AppError::bad_request(
            "NO_EMBEDDING_MODEL",
            "Configure an embedding model before triggering a re-embed",
        )
        .into());
    };
    let model = Repos
        .llm_model
        .get_by_id(model_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("LlmModel"))?;

    // Probe dim + spawn worker. Same pattern as the auto-trigger in
    // update_admin_settings, factored to also handle the same-id
    // case (e.g. "Re-embed now" button, post-stale-rows recovery).
    let pool = Repos.memory.pool_clone();
    let model_name = model.name.clone();
    tokio::spawn(async move {
        let dim = match crate::modules::memory::engine::dispatch::embed(
            model_id,
            "dimension probe",
        )
        .await
        {
            Ok(v) => v.len() as i32,
            Err(e) => {
                tracing::warn!(
                    "memory.admin: dimension probe failed for {model_id}: {e} — keeping existing dim"
                );
                return;
            }
        };
        super::embedding_worker::reembed_all(pool, model_id, model_name, dim).await;
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "ok": true, "started": true })),
    ))
}

pub fn trigger_reembed_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminManage,)>(op)
        .id("MemoryAdmin.reembed")
        .tag("Memory")
        .summary("Trigger a re-embed of all memories using the current embedding model")
        .response::<202, Json<serde_json::Value>>()
}

// ============================================================================
// FTS rebuild — dictionary swap path. Because `user_memories.content_tsv` is
// a GENERATED ALWAYS STORED column with the dictionary baked into the
// expression, swapping dictionaries means dropping + re-adding the column.
// We do that inside one transaction guarded by a process-global advisory
// lock so two rebuilds can't race; the table is never left in a partial
// state. Long-running on large stores — caller polls /status until the
// `fts_rebuild_completed_at` timestamp is set.
// ============================================================================

#[debug_handler]
pub async fn trigger_fts_rebuild(
    _auth: RequirePermissions<(MemoryAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<super::models::FtsRebuildRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // Allowlist gate BEFORE we touch the DB. Defense in depth against
    // the eventual fingers-on-keyboard typo; the DDL below interpolates
    // the dictionary name directly (sqlx bind params don't work in
    // `GENERATED AS to_tsvector($1, content)`) so this allowlist + the
    // CHECK constraint are the two layers protecting us from injection.
    if !super::models::is_valid_fts_dictionary(&body.dictionary) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "fts_dictionary not in allowlist",
        )
        .into());
    }

    // Short-circuit same-dictionary rebuild — the GENERATED expression
    // would produce identical content and the DROP/ADD/CREATE INDEX is
    // a wasted full-table rewrite. Admins occasionally want this to
    // recover from a corrupt index; let them use a separate "reindex"
    // affordance for that, not this endpoint.
    let prior = Repos.memory.get_admin_settings().await?;
    if body.dictionary == prior.fts_dictionary {
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "started": false,
                "reason": "dictionary unchanged",
            })),
        ));
    }

    // Claim the rebuild slot via a single CAS UPDATE. If another rebuild
    // is already running we get false and return 409 — no DDL spawned,
    // no leaked timestamps.
    let claimed = Repos.memory.try_claim_fts_rebuild().await?;
    if !claimed {
        return Err(AppError::new(
            axum::http::StatusCode::CONFLICT,
            "FTS_REBUILD_IN_PROGRESS",
            "an FTS rebuild is already running; wait for it to complete",
        )
        .into());
    }

    // Defense in depth: between the CAS claim and the spawn there are
    // only infallible CPU ops, but a runtime drop / OOM / panic in that
    // window would leak the slot. Take ownership of a guard that fires
    // `clear_fts_rebuild_marker` on Drop unless explicitly disarmed —
    // we disarm after `tokio::spawn` succeeds, transferring slot
    // ownership to the worker.
    struct ClaimGuard {
        armed: bool,
    }
    impl Drop for ClaimGuard {
        fn drop(&mut self) {
            if !self.armed {
                return;
            }
            // Only spawn the cleanup if a tokio runtime is reachable.
            // During graceful shutdown the runtime may already be gone
            // (test teardown, SIGTERM); spawning then panics-in-drop
            // which aborts the process. Best-effort: if there's no
            // runtime, the marker stays set and the operator handles
            // it manually.
            if tokio::runtime::Handle::try_current().is_ok() {
                tokio::spawn(async {
                    let _ = Repos.memory.clear_fts_rebuild_marker().await;
                });
            }
        }
    }
    let mut guard = ClaimGuard { armed: true };

    // Drive the UI's in_progress=true flip without waiting for a poll
    // cycle — the CAS-claim above already wrote started_at, so the
    // refetch will see it.
    sync_publish(
        SyncEntity::MemoryAdminSettings,
        SyncAction::Update,
        uuid::Uuid::nil(),
        Audience::perm::<MemoryAdminRead>(),
        origin.0,
    );

    let pool = Repos.memory.pool_clone();
    let dict = body.dictionary.clone();

    tokio::spawn(async move {
        let result: Result<(), sqlx::Error> = async {
            let mut tx = pool.begin().await?;

            // DROP + ADD GENERATED column. The dictionary name is
            // interpolated from the allowlist gate above — never from
            // the request directly. The ALTER takes an
            // AccessExclusiveLock on user_memories for the duration of
            // the rewrite, which is the correct serialization primitive
            // here — concurrent rebuilds are already rejected by the
            // CAS claim above.
            sqlx::query("ALTER TABLE user_memories DROP COLUMN content_tsv")
                .execute(&mut *tx)
                .await?;
            let add_col = format!(
                "ALTER TABLE user_memories ADD COLUMN content_tsv tsvector \
                 GENERATED ALWAYS AS (to_tsvector('{}', content)) STORED",
                dict
            );
            sqlx::query(&add_col).execute(&mut *tx).await?;
            sqlx::query(
                "CREATE INDEX idx_user_memories_tsv ON user_memories USING GIN (content_tsv)",
            )
            .execute(&mut *tx)
            .await?;

            // Persist the swap + completed-at marker in the same
            // transaction. The CAS-claimed `started_at` is also
            // visible to other connections only after this commit, so
            // status readers see a clean transition: started_at set →
            // completed_at set (no torn-write window).
            if let Err(e) = Repos.memory.complete_fts_rebuild(&mut tx, &dict).await {
                tracing::error!(
                    "memory.fts_rebuild: complete_fts_rebuild failed: {}",
                    e
                );
                return Err(sqlx::Error::Configuration(e.to_string().into()));
            }
            tx.commit().await?;
            Ok(())
        }
        .await;

        match result {
            Ok(()) => tracing::info!(
                "memory.fts_rebuild: rebuilt content_tsv with dictionary '{}'",
                dict
            ),
            Err(e) => {
                tracing::error!(
                    "memory.fts_rebuild: rebuild for dictionary '{}' failed: {}",
                    dict,
                    e
                );
                // Release the claim so the next attempt can run. Best-
                // effort — a DB outage at this point means the row
                // stays claimed until the operator inspects, which is
                // the correct fail-loud behavior for total DB loss.
                if let Err(cleanup_err) =
                    Repos.memory.clear_fts_rebuild_marker().await
                {
                    tracing::error!(
                        "memory.fts_rebuild: failed to clear marker after error: {}",
                        cleanup_err
                    );
                }
            }
        }
        // Drive a UI refetch on both success (so dict / completed_at
        // land) and failure (so started_at clearing lands). origin=None
        // because the worker is detached from the originating request.
        sync_publish(
            SyncEntity::MemoryAdminSettings,
            SyncAction::Update,
            uuid::Uuid::nil(),
            Audience::perm::<MemoryAdminRead>(),
            None,
        );
    });

    // Worker owns the slot now — disarm the guard so it doesn't
    // clear the marker out from under the in-flight rebuild.
    guard.armed = false;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "ok": true, "started": true })),
    ))
}

pub fn trigger_fts_rebuild_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminManage,)>(op)
        .id("MemoryAdmin.ftsRebuild")
        .tag("Memory")
        .summary("Rebuild user_memories.content_tsv with a new dictionary")
        .description(
            "Long-running; client should poll GET /memory/admin/fts/rebuild/status until \
             completed_at is set.",
        )
        .response::<202, Json<serde_json::Value>>()
}

#[debug_handler]
pub async fn get_fts_rebuild_status(
    _auth: RequirePermissions<(MemoryAdminRead,)>,
) -> ApiResult<Json<super::models::FtsRebuildStatus>> {
    let admin = Repos.memory.get_admin_settings().await?;
    let in_progress = admin.fts_rebuild_started_at.is_some()
        && admin.fts_rebuild_completed_at.is_none();
    Ok((
        StatusCode::OK,
        Json(super::models::FtsRebuildStatus {
            in_progress,
            started_at: admin.fts_rebuild_started_at,
            completed_at: admin.fts_rebuild_completed_at,
        }),
    ))
}

pub fn get_fts_rebuild_status_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminRead,)>(op)
        .id("MemoryAdmin.ftsRebuildStatus")
        .tag("Memory")
        .summary("Read FTS rebuild status")
        .response::<200, Json<super::models::FtsRebuildStatus>>()
}

// ============================================================================
// Test-only hooks (Tier-5 real-LLM integration tests).
//
// Gated behind `#[cfg(debug_assertions)]` so release builds physically
// don't ship these routes. They expose the extractor pipeline that
// normally fires from `after_llm_call` so a test can
// trigger them synchronously via HTTP without needing to drive a
// full chat conversation. Admin-perm-gated for paranoia in case a
// debug binary ever runs in a quasi-production setting.
// ============================================================================

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestExtractRequest {
    pub user_id: Uuid,
    pub user_message: String,
    pub assistant_message: String,
}

#[cfg(debug_assertions)]
#[debug_handler]
pub async fn test_extract(
    _auth: RequirePermissions<(MemoryAdminManage,)>,
    Json(body): Json<TestExtractRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::modules::memory::engine::extractor::extract_and_persist(
        body.user_id,
        body.user_message,
        body.assistant_message,
        None,
    )
    .await;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

#[cfg(debug_assertions)]
pub fn test_extract_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminManage,)>(op)
        .id("MemoryTest.extract")
        .tag("Memory")
        .summary("Test-only: trigger extraction synchronously (debug builds)")
        .response::<200, Json<serde_json::Value>>()
}

// `test_summarize` debug handler moved to the `summarization` module
// (migration 91 — `POST /api/_test/summarization/refresh`).
