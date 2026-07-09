//! Document-RAG repository — `file_chunks` + `file_rag_admin_settings`.
//!
//! Chunk rows are keyed by `file_id` (one logical index per file = head
//! version); `reindex_chunks` (a per-file-serialized DELETE+INSERT transaction)
//! is the re-index primitive. Retrieval SQL (vector/fts/hybrid) lives in `retrieval.rs`
//! because it needs runtime-formatted halfvec operators the `query!` macro
//! can't verify; everything embedding-free is a checked macro here.

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{ChunkDraft, FileRagAdminSettings, IndexTarget};
use crate::common::AppError;
use pgvector::HalfVector;

#[derive(Clone, Debug)]
pub struct FileRagRepository {
    pool: PgPool,
}

impl FileRagRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Cheap pool clone for callers that run dynamic SQL (the retriever's
    /// halfvec top-K, the rebuild worker's ALTER) outside a typed method.
    pub fn pool_clone(&self) -> PgPool {
        self.pool.clone()
    }

    // ── file_rag_admin_settings (single row, id=1) ──────────────────────

    pub async fn get_admin_settings(&self) -> Result<FileRagAdminSettings, AppError> {
        let row = sqlx::query_as!(
            FileRagAdminSettings,
            r#"
            SELECT
                id,
                enabled,
                embedding_model_id,
                embedding_dimensions,
                chunk_chars,
                chunk_overlap_chars,
                max_chunks_per_file,
                default_top_k,
                cosine_threshold,
                semantic_enabled,
                fts_enabled,
                fts_dictionary,
                fts_rrf_k,
                fts_candidate_multiplier,
                fts_min_rank,
                reranker_model_id,
                rerank_enabled,
                rerank_candidate_k,
                updated_at as "updated_at: _"
            FROM file_rag_admin_settings
            WHERE id = 1
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Partial update of the singleton. `embedding_model_id` uses the
    /// `Option<Option<Uuid>>` split (absent / clear / set). `embedding_dimensions`
    /// is handler-supplied (probe-derived from the chosen model), never typed.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_admin_settings(
        &self,
        enabled: Option<bool>,
        embedding_model_id: Option<Option<Uuid>>,
        embedding_dimensions: Option<i32>,
        chunk_chars: Option<i32>,
        chunk_overlap_chars: Option<i32>,
        max_chunks_per_file: Option<i32>,
        default_top_k: Option<i16>,
        cosine_threshold: Option<f32>,
        semantic_enabled: Option<bool>,
        fts_enabled: Option<bool>,
        fts_rrf_k: Option<i32>,
        fts_candidate_multiplier: Option<i32>,
        fts_min_rank: Option<f32>,
        reranker_model_id: Option<Option<Uuid>>,
        rerank_enabled: Option<bool>,
        rerank_candidate_k: Option<i32>,
    ) -> Result<FileRagAdminSettings, AppError> {
        let embedding_set = embedding_model_id.is_some();
        let embedding_val = embedding_model_id.flatten();
        let reranker_set = reranker_model_id.is_some();
        let reranker_val = reranker_model_id.flatten();

        let row = sqlx::query_as!(
            FileRagAdminSettings,
            r#"
            UPDATE file_rag_admin_settings
            SET enabled                  = COALESCE($1, enabled),
                embedding_model_id       = CASE WHEN $2::bool THEN $3 ELSE embedding_model_id END,
                embedding_dimensions     = COALESCE($4, embedding_dimensions),
                chunk_chars              = COALESCE($5, chunk_chars),
                chunk_overlap_chars      = COALESCE($6, chunk_overlap_chars),
                max_chunks_per_file      = COALESCE($7, max_chunks_per_file),
                default_top_k            = COALESCE($8, default_top_k),
                cosine_threshold         = COALESCE($9, cosine_threshold),
                semantic_enabled         = COALESCE($10, semantic_enabled),
                fts_enabled              = COALESCE($11, fts_enabled),
                fts_rrf_k                = COALESCE($12, fts_rrf_k),
                fts_candidate_multiplier = COALESCE($13, fts_candidate_multiplier),
                fts_min_rank             = COALESCE($14, fts_min_rank),
                reranker_model_id        = CASE WHEN $15::bool THEN $16 ELSE reranker_model_id END,
                rerank_enabled           = COALESCE($17, rerank_enabled),
                rerank_candidate_k       = COALESCE($18, rerank_candidate_k),
                updated_at               = NOW()
            WHERE id = 1
            RETURNING
                id,
                enabled,
                embedding_model_id,
                embedding_dimensions,
                chunk_chars,
                chunk_overlap_chars,
                max_chunks_per_file,
                default_top_k,
                cosine_threshold,
                semantic_enabled,
                fts_enabled,
                fts_dictionary,
                fts_rrf_k,
                fts_candidate_multiplier,
                fts_min_rank,
                reranker_model_id,
                rerank_enabled,
                rerank_candidate_k,
                updated_at as "updated_at: _"
            "#,
            enabled,
            embedding_set,
            embedding_val,
            embedding_dimensions,
            chunk_chars,
            chunk_overlap_chars,
            max_chunks_per_file,
            default_top_k,
            cosine_threshold,
            semantic_enabled,
            fts_enabled,
            fts_rrf_k,
            fts_candidate_multiplier,
            fts_min_rank,
            reranker_set,
            reranker_val,
            rerank_enabled,
            rerank_candidate_k,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    // ── file_chunks ─────────────────────────────────────────────────────

    /// Atomically replace a file's chunks. Serializes per-file on an advisory
    /// xact lock (auto-released on commit/rollback), then DELETE-then-INSERT in
    /// ONE transaction, so: (a) retrieval never sees a half-built/empty index
    /// mid-reindex, (b) concurrent re-index of the same file can't interleave
    /// into mixed/duplicate chunks, and (c) a failed insert rolls back the
    /// delete (no permanent wipe). New chunks carry NO embedding (FTS works
    /// immediately via the GENERATED `content_tsv`; the vector arm fills
    /// `embedding` in afterward). An empty `drafts` clears the index for a file
    /// that lost its extractable text. (File deletion is handled by the
    /// `file_id` ON DELETE CASCADE, not this path.)
    pub async fn reindex_chunks(
        &self,
        file_id: Uuid,
        user_id: Uuid,
        blob_version_id: Uuid,
        version: i32,
        drafts: &[ChunkDraft],
    ) -> Result<u64, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Per-file lock. The string prefix namespaces it so it can't collide
        // with any other advisory lock keyed on a bare file_id hash.
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtext('file_rag_reindex:' || $1::text)::bigint)",
        )
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        sqlx::query!("DELETE FROM file_chunks WHERE file_id = $1", file_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;

        let inserted = if drafts.is_empty() {
            0
        } else {
            let page_numbers: Vec<i32> = drafts.iter().map(|d| d.page_number).collect();
            let chunk_indices: Vec<i32> = drafts.iter().map(|d| d.chunk_index).collect();
            let char_starts: Vec<i32> = drafts.iter().map(|d| d.char_start).collect();
            let char_ends: Vec<i32> = drafts.iter().map(|d| d.char_end).collect();
            let contents: Vec<String> = drafts.iter().map(|d| d.content.clone()).collect();
            let res = sqlx::query!(
                r#"
                INSERT INTO file_chunks
                    (file_id, user_id, blob_version_id, version,
                     page_number, chunk_index, char_start, char_end, content)
                SELECT $1, $2, $3, $4, pn, ci, cs, ce, ct
                FROM UNNEST($5::int[], $6::int[], $7::int[], $8::int[], $9::text[])
                     AS t(pn, ci, cs, ce, ct)
                "#,
                file_id,
                user_id,
                blob_version_id,
                version,
                &page_numbers,
                &chunk_indices,
                &char_starts,
                &char_ends,
                &contents,
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            res.rows_affected()
        };

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(inserted)
    }

    /// Chunks of one file that still need an embedding for `model_name`
    /// (NULL embedding or embedded with a different model). Drives the
    /// per-file post-ingest embed pass.
    pub async fn chunks_needing_embedding_for_file(
        &self,
        file_id: Uuid,
        model_name: &str,
    ) -> Result<Vec<(Uuid, Uuid, String)>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, user_id, content
            FROM file_chunks
            WHERE file_id = $1
              AND (embedding IS NULL OR embedding_model IS DISTINCT FROM $2)
            ORDER BY chunk_index
            "#,
            file_id,
            model_name,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| (r.id, r.user_id, r.content)).collect())
    }

    /// A deployment-wide batch of chunks needing (re-)embedding — drives the
    /// dimension-rebuild / re-embed worker. Returns `(id, user_id, content)`.
    pub async fn chunks_needing_embedding(
        &self,
        model_name: &str,
        limit: i64,
    ) -> Result<Vec<(Uuid, Uuid, String)>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, user_id, content
            FROM file_chunks
            WHERE embedding IS NULL OR embedding_model IS DISTINCT FROM $1
            LIMIT $2
            "#,
            model_name,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| (r.id, r.user_id, r.content)).collect())
    }

    /// Write one chunk's embedding. Untyped query (like memory's worker) —
    /// the `query!` macro can't verify the `halfvec` parameter type. The
    /// `user_id` predicate is defense-in-depth parity with memory's worker
    /// (the `id` PK is already unambiguous).
    pub async fn set_chunk_embedding(
        &self,
        id: Uuid,
        user_id: Uuid,
        embedding: &HalfVector,
        model_name: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE file_chunks SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4",
        )
        .bind(embedding)
        .bind(model_name)
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Files that have extracted text but no chunks yet — the backfill
    /// work-list. Joined to the head version for its blob/version. `exclude`
    /// drops already-attempted ids so the sliding window advances past files
    /// that have text pages but yield zero chunks (scanned/whitespace-only
    /// PDFs) — they match the predicate forever, and without this they would
    /// wall off newer indexable files behind them.
    pub async fn files_with_text_missing_chunks(
        &self,
        limit: i64,
        exclude: &[Uuid],
    ) -> Result<Vec<IndexTarget>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                f.id              AS "file_id!",
                f.user_id         AS "user_id!",
                fv.blob_version_id AS "blob_version_id!",
                fv.version        AS "version!",
                f.text_page_count AS "text_page_count!"
            FROM files f
            JOIN file_versions fv ON fv.id = f.current_version_id
            WHERE f.text_page_count > 0
              AND NOT EXISTS (SELECT 1 FROM file_chunks fc WHERE fc.file_id = f.id)
              AND f.id <> ALL($2::uuid[])
            ORDER BY f.created_at
            LIMIT $1
            "#,
            limit,
            exclude,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows
            .into_iter()
            .map(|r| IndexTarget {
                file_id: r.file_id,
                user_id: r.user_id,
                blob_version_id: r.blob_version_id,
                version: r.version,
                text_page_count: r.text_page_count,
            })
            .collect())
    }
}
