//! REST handlers for `/api/file-rag/admin-settings` (+ reembed + backfill).
//!
//! All admin-gated. The PUT derives `embedding_dimensions` by probe-embedding
//! the chosen model (never trusting a typed number), and spawns the re-embed /
//! HNSW-rebuild worker when a model is set or changed.

use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::Repos,
    modules::{
        file_rag::{
            embed_worker, ingest,
            models::{FileRagAdminSettings, UpdateFileRagAdminSettingsRequest},
            permissions::{FileRagAdminManage, FileRagAdminRead},
        },
        memory::engine::dispatch,
        permissions::{RequirePermissions, with_permission},
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
    },
};

/// Largest embedding dimension the HNSW halfvec index supports.
const MAX_EMBED_DIM: i32 = 4000;

#[debug_handler]
pub async fn get_admin_settings(
    _auth: RequirePermissions<(FileRagAdminRead,)>,
) -> ApiResult<Json<FileRagAdminSettings>> {
    let row = Repos.file_rag.get_admin_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FileRagAdminRead,)>(op)
        .id("FileRagAdmin.get")
        .tag("FileRag")
        .summary("Read Document-RAG admin settings")
        .response::<200, Json<FileRagAdminSettings>>()
}

#[debug_handler]
pub async fn update_admin_settings(
    _auth: RequirePermissions<(FileRagAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateFileRagAdminSettingsRequest>,
) -> ApiResult<Json<FileRagAdminSettings>> {
    let bad = |m: &str| -> AppError { AppError::bad_request("VALIDATION_ERROR", m.to_string()) };

    // Range validations mirror the DB CHECK constraints — return a clean 400
    // instead of a raw 500 from the database.
    if let Some(k) = body.default_top_k {
        // Ceiling matches the per-call `semantic_search` clamp (1..=50) so an
        // admin's default can't silently exceed what a single call can return.
        if !(1..=50).contains(&k) {
            return Err(bad("default_top_k out of range (1..=50)").into());
        }
    }
    if let Some(t) = body.cosine_threshold {
        if !(0.0..=2.0).contains(&t) {
            return Err(bad("cosine_threshold out of range (0.0..=2.0)").into());
        }
    }
    if let Some(c) = body.chunk_chars {
        if !(200..=8000).contains(&c) {
            return Err(bad("chunk_chars out of range (200..=8000)").into());
        }
    }
    if let Some(o) = body.chunk_overlap_chars {
        if o < 0 {
            return Err(bad("chunk_overlap_chars must be >= 0").into());
        }
    }
    if let Some(m) = body.max_chunks_per_file {
        if m <= 0 {
            return Err(bad("max_chunks_per_file must be > 0").into());
        }
    }
    if let Some(k) = body.fts_rrf_k {
        if !(1..=1000).contains(&k) {
            return Err(bad("fts_rrf_k out of range (1..=1000)").into());
        }
    }
    if let Some(m) = body.fts_candidate_multiplier {
        if !(1..=20).contains(&m) {
            return Err(bad("fts_candidate_multiplier out of range (1..=20)").into());
        }
    }
    if let Some(r) = body.fts_min_rank {
        if !(0.0..=1.0).contains(&r) {
            return Err(bad("fts_min_rank out of range (0.0..=1.0)").into());
        }
    }
    if let Some(k) = body.rerank_candidate_k {
        if !(1..=200).contains(&k) {
            return Err(bad("rerank_candidate_k out of range (1..=200)").into());
        }
    }
    // Retrieval / KB limits (mirror the DB CHECK constraints — clean 400 not 500).
    if let Some(n) = body.kb_max_documents {
        if !(1..=100_000).contains(&n) {
            return Err(bad("kb_max_documents out of range (1..=100000)").into());
        }
    }
    if let Some(n) = body.search_max_hit_chars {
        if !(100..=100_000).contains(&n) {
            return Err(bad("search_max_hit_chars out of range (100..=100000)").into());
        }
    }
    if let Some(n) = body.search_snippet_chars {
        if !(20..=4000).contains(&n) {
            return Err(bad("search_snippet_chars out of range (20..=4000)").into());
        }
    }
    if let Some(n) = body.search_max_top_k {
        if !(1..=500).contains(&n) {
            return Err(bad("search_max_top_k out of range (1..=500)").into());
        }
    }
    // Probe: a model set as the reranker MUST carry the `rerank` capability.
    if let Some(Some(model_id)) = body.reranker_model_id {
        let model = Repos
            .llm_model
            .get_by_id(model_id)
            .await
            .map_err(AppError::database_error)?
            .ok_or_else(|| bad("reranker model not found"))?;
        if let Some(reason) = crate::modules::memory::engine::capability::rerank_unsupported_reason(
            &model.name,
            &model.capabilities,
        ) {
            return Err(bad(&format!("INVALID_RERANK_MODEL: {reason}")).into());
        }
    }

    // Cross-field: overlap must stay below the (possibly new) chunk size.
    let current = Repos.file_rag.get_admin_settings().await?;
    let eff_chunk = body.chunk_chars.unwrap_or(current.chunk_chars);
    let eff_overlap = body.chunk_overlap_chars.unwrap_or(current.chunk_overlap_chars);
    if eff_overlap >= eff_chunk {
        return Err(bad("chunk_overlap_chars must be < chunk_chars").into());
    }

    // Probe-embed to DERIVE the dimension when a model is (re)set — never trust
    // a typed number. The probe also validates the model is a working embedder.
    // NOTE: we do NOT write `embedding_dimensions` here — the rebuild worker is
    // its sole owner. It reads the OLD value, and only if it differs from the
    // probed target does it ALTER the column + write the new value. Writing it
    // here would make the worker see current == target and skip the ALTER,
    // leaving a halfvec(old) column that rejects new-dim vectors.
    let mut spawn_reembed: Option<(Uuid, i32)> = None;
    if let Some(Some(model_id)) = body.embedding_model_id {
        let v = dispatch::embed(model_id, "dimension probe").await.map_err(|e| {
            // Log the underlying provider error server-side only; the client
            // gets a generic message (matches memory's policy — don't leak
            // provider URLs / upstream response bodies into the HTTP response).
            tracing::warn!("file_rag: embedding-model probe failed for {model_id}: {e}");
            AppError::bad_request(
                "INVALID_EMBEDDING_MODEL",
                "the selected model could not produce an embedding; check that it is an \
                 embedding model and that its provider is configured",
            )
        })?;
        let dim = v.len() as i32;
        if dim < 1 || dim > MAX_EMBED_DIM {
            return Err(AppError::bad_request(
                "UNSUPPORTED_DIMENSION",
                format!(
                    "embedding model returned {dim} dims; the HNSW halfvec index supports 1..={MAX_EMBED_DIM}"
                ),
            )
            .into());
        }
        spawn_reembed = Some((model_id, dim));
    }

    let updated = Repos
        .file_rag
        .update_admin_settings(
            body.enabled,
            body.embedding_model_id,
            None, // embedding_dimensions is owned by the rebuild worker (see above)
            body.chunk_chars,
            body.chunk_overlap_chars,
            body.max_chunks_per_file,
            body.default_top_k,
            body.cosine_threshold,
            body.semantic_enabled,
            body.fts_enabled,
            body.fts_rrf_k,
            body.fts_candidate_multiplier,
            body.fts_min_rank,
            body.reranker_model_id,
            body.rerank_enabled,
            body.rerank_candidate_k,
            body.kb_max_documents,
            body.search_max_hit_chars,
            body.search_snippet_chars,
            body.search_max_top_k,
        )
        .await?;

    // Re-embed (and ALTER the column if the dimension changed) in the
    // background whenever a model is set/changed.
    if let Some((model_id, dim)) = spawn_reembed {
        let pool = Repos.file_rag.pool_clone();
        tokio::spawn(async move { embed_worker::reembed_all(pool, model_id, dim).await });
    }

    // Cross-device sync: notify admins so their settings page refetches.
    sync_publish(
        SyncEntity::FileRagAdminSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<FileRagAdminRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(updated)))
}

pub fn update_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FileRagAdminManage,)>(op)
        .id("FileRagAdmin.update")
        .tag("FileRag")
        .summary("Update Document-RAG admin settings")
        .response::<200, Json<FileRagAdminSettings>>()
}

/// Response for the fire-and-forget admin triggers.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TriggerResponse {
    pub status: String,
}

#[debug_handler]
pub async fn reembed(
    _auth: RequirePermissions<(FileRagAdminManage,)>,
) -> ApiResult<Json<TriggerResponse>> {
    let admin = Repos.file_rag.get_admin_settings().await?;
    let Some(model_id) = admin.embedding_model_id else {
        return Err(AppError::bad_request(
            "NO_EMBEDDING_MODEL",
            "no embedding model is configured; nothing to re-embed",
        )
        .into());
    };
    let pool = Repos.file_rag.pool_clone();
    let dim = admin.embedding_dimensions;
    tokio::spawn(async move { embed_worker::reembed_all(pool, model_id, dim).await });
    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerResponse {
            status: "re-embed started".to_string(),
        }),
    ))
}

pub fn reembed_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FileRagAdminManage,)>(op)
        .id("FileRagAdmin.reembed")
        .tag("FileRag")
        .summary("Re-embed all document chunks with the configured model")
        .response::<202, Json<TriggerResponse>>()
}

#[debug_handler]
pub async fn backfill(
    _auth: RequirePermissions<(FileRagAdminManage,)>,
) -> ApiResult<Json<TriggerResponse>> {
    tokio::spawn(async move { ingest::run_backfill().await });
    Ok((
        StatusCode::ACCEPTED,
        Json(TriggerResponse {
            status: "backfill started".to_string(),
        }),
    ))
}

pub fn backfill_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(FileRagAdminManage,)>(op)
        .id("FileRagAdmin.backfill")
        .tag("FileRag")
        .summary("Index pre-existing files that have text but no chunks")
        .response::<202, Json<TriggerResponse>>()
}
