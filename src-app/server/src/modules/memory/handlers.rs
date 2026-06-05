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
        sync::{SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
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
    if content.len() > MAX_CONTENT_LEN {
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
        )
        .await?;
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Create,
        row.id,
        Some(auth.user.id),
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
        if c.len() > MAX_CONTENT_LEN {
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
        Some(auth.user.id),
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
        Some(auth.user.id),
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
        Some(auth.user.id),
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
        Some(auth.user.id),
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

    // Prompt-template validation: a non-empty override MUST contain
    // the placeholders the summarizer interpolates. Otherwise
    // summarization would silently produce broken prompts.
    // `Some(None)` (clear back to default) and `Some(Some(""))`
    // (clear-via-empty) both skip validation — those reset to the
    // compiled-in default, which is always valid.
    if let Some(Some(s)) = body.full_summary_prompt.as_ref() {
        if !s.is_empty() && !s.contains("{transcript}") {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "full_summary_prompt must contain the {transcript} placeholder",
            )
            .into());
        }
    }
    if let Some(Some(s)) = body.incremental_summary_prompt.as_ref() {
        if !s.is_empty()
            && (!s.contains("{previous_summary}") || !s.contains("{new_transcript}"))
        {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "incremental_summary_prompt must contain both {previous_summary} and {new_transcript} placeholders",
            )
            .into());
        }
    }
    // Normalize: treat Some(Some("")) as Some(None) (clear). Otherwise
    // the empty string would be written verbatim and the summarizer
    // would short-circuit to "empty prompt" without falling back to
    // the default.
    let full_summary_prompt = body.full_summary_prompt.map(|outer| {
        outer.and_then(|s| if s.is_empty() { None } else { Some(s) })
    });
    let incremental_summary_prompt = body.incremental_summary_prompt.map(|outer| {
        outer.and_then(|s| if s.is_empty() { None } else { Some(s) })
    });

    // Snapshot the current embedding model id so we can detect a swap
    // and trigger the re-embed worker if it changed.
    let prior = Repos.memory.get_admin_settings().await?;
    let prior_model_id = prior.embedding_model_id;

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
            body.summarize_after_n_messages,
            body.summarizer_keep_recent,
            full_summary_prompt,
            incremental_summary_prompt,
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
// Test-only hooks (Tier-5 real-LLM integration tests).
//
// Gated behind `#[cfg(debug_assertions)]` so release builds physically
// don't ship these routes. They expose the extractor/summarizer
// pipelines that normally fire from `after_llm_call` so a test can
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

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestSummarizeRequest {
    pub branch_id: Uuid,
    pub model_id: Uuid,
}

#[cfg(debug_assertions)]
#[debug_handler]
pub async fn test_summarize(
    _auth: RequirePermissions<(MemoryAdminManage,)>,
    Json(body): Json<TestSummarizeRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::modules::memory::engine::summarizer::refresh_summary(
        body.branch_id,
        body.model_id,
    )
    .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

#[cfg(debug_assertions)]
pub fn test_summarize_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MemoryAdminManage,)>(op)
        .id("MemoryTest.summarize")
        .tag("Memory")
        .summary("Test-only: trigger summary refresh synchronously (debug builds)")
        .response::<200, Json<serde_json::Value>>()
}
