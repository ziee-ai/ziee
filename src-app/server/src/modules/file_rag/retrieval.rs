//! Document-RAG retrieval — vector ⊕ FTS hybrid over `file_chunks`, scoped by
//! `file_id = ANY($1)` (the conversation's available files, resolved upstream).
//!
//! Adapted from `memory::chat_extension::retriever` (same 4-arm decision tree,
//! same RRF fusion formula `1/(k+rank)`), but over chunks with span-level
//! provenance instead of memory rows. The arms use runtime-prepared queries
//! (`query_as`) because the `halfvec <=>` operator and `regconfig` cast aren't
//! verifiable by the `query!` macro.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use uuid::Uuid;

use super::models::{FileRagAdminSettings, RetrievalMode, SemanticHit};
use crate::common::AppError;
use crate::core::Repos;
use crate::modules::memory::engine::dispatch::embed;
use pgvector::HalfVector;

/// A chunk row plus the arm's raw relevance metric (`metric`): cosine distance
/// for the vector arm, `ts_rank_cd` for the FTS arm. Carries the full
/// provenance the grounding layer needs.
#[derive(Debug, Clone, sqlx::FromRow)]
struct ScoredChunkRow {
    id: Uuid,
    file_id: Uuid,
    blob_version_id: Uuid,
    version: i32,
    page_number: i32,
    char_start: i32,
    char_end: i32,
    content: String,
    metric: f64,
}

impl ScoredChunkRow {
    fn into_hit(self, score: f64) -> SemanticHit {
        SemanticHit {
            file_id: self.file_id,
            blob_version_id: self.blob_version_id,
            version: self.version,
            page_number: self.page_number,
            char_start: self.char_start,
            char_end: self.char_end,
            content: self.content,
            score,
        }
    }
}

/// Result of a search: ordered hits, which arms produced them, and whether more
/// matches existed beyond `top_k` (detected by fetching one extra row).
pub struct SearchResult {
    pub hits: Vec<SemanticHit>,
    pub mode: RetrievalMode,
    pub truncated: bool,
}

/// Which retrieval arm a given admin config selects — the static half of the
/// decision. (The dynamic half is the embed-failure fallback inside
/// `semantic_search`: Hybrid degrades to FTS, Vector to empty.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    Hybrid,
    Vector,
    Fts,
    None,
}

/// `has_vector` = `semantic_enabled AND embedding_model_id IS NOT NULL`.
fn plan_arm(has_vector: bool, fts_enabled: bool) -> Arm {
    match (has_vector, fts_enabled) {
        (true, true) => Arm::Hybrid,
        (true, false) => Arm::Vector,
        (false, true) => Arm::Fts,
        (false, false) => Arm::None,
    }
}

/// Single entry point — the 4-arm decision tree (vector availability ×
/// fts_enabled). `scope_ids` are the file ids the caller is allowed to search
/// (already permission-resolved); `user_id` is a redundant defense-in-depth
/// scope (every scope_id already belongs to this user). An empty scope or blank
/// query short-circuits.
pub async fn semantic_search(
    scope_ids: &[Uuid],
    user_id: Uuid,
    query: &str,
    top_k: i64,
    admin: &FileRagAdminSettings,
) -> Result<SearchResult, AppError> {
    if scope_ids.is_empty() || query.trim().is_empty() {
        return Ok(SearchResult {
            hits: Vec::new(),
            mode: RetrievalMode::None,
            truncated: false,
        });
    }
    let dict = admin.fts_dictionary.as_str();
    let min_rank = admin.fts_min_rank;
    // Collapse "semantic disabled" into "no embedding model" so both reasons
    // for vector-arm unavailability flow through the same branches.
    let vector_emb_id = if admin.semantic_enabled {
        admin.embedding_model_id
    } else {
        None
    };
    // Fetch one extra hit so `truncated` is precise rather than a heuristic.
    let probe = top_k.saturating_add(1);

    let (mut hits, mode): (Vec<SemanticHit>, RetrievalMode) =
        match (plan_arm(vector_emb_id.is_some(), admin.fts_enabled), vector_emb_id) {
            (Arm::Hybrid, Some(emb_id)) => match embed(emb_id, query).await {
                Ok(v) => (
                    hybrid_search(
                        scope_ids,
                        user_id,
                        &HalfVector::from_f32_slice(&v),
                        admin.cosine_threshold,
                        query,
                        probe,
                        dict,
                        min_rank,
                        admin.fts_rrf_k,
                        admin.fts_candidate_multiplier,
                    )
                    .await?,
                    RetrievalMode::Hybrid,
                ),
                Err(e) => {
                    tracing::warn!("file_rag.search: embed failed ({e}); FTS-only fallback");
                    (
                        fts_search(scope_ids, user_id, query, probe, dict, min_rank).await?,
                        RetrievalMode::Fts,
                    )
                }
            },
            (Arm::Vector, Some(emb_id)) => match embed(emb_id, query).await {
                Ok(v) => (
                    vector_search(
                        scope_ids,
                        user_id,
                        &HalfVector::from_f32_slice(&v),
                        admin.cosine_threshold,
                        probe,
                    )
                    .await?,
                    RetrievalMode::Vector,
                ),
                Err(e) => {
                    tracing::warn!(
                        "file_rag.search: embed failed ({e}); fts_enabled=false → empty (no fallback)"
                    );
                    (Vec::new(), RetrievalMode::Vector)
                }
            },
            (Arm::Fts, _) => (
                fts_search(scope_ids, user_id, query, probe, dict, min_rank).await?,
                RetrievalMode::Fts,
            ),
            // Arm::None, plus the logically-impossible vector-arm-without-a-model
            // combos (plan_arm only returns Hybrid/Vector when has_vector is true).
            _ => (Vec::new(), RetrievalMode::None),
        };

    let truncated = hits.len() as i64 > top_k;
    hits.truncate(top_k.max(0) as usize);
    Ok(SearchResult {
        hits,
        mode,
        truncated,
    })
}

const SELECT_COLS: &str =
    "id, file_id, blob_version_id, version, page_number, char_start, char_end, content";

/// Vector (cosine) arm. `metric` = cosine distance; hit score = 1 − distance.
/// Test-only: number of vector-arm hits for a raw query vector. Re-exported via
/// `ziee::file_rag_search` so the concurrent-search-during-embed (NULL embedding
/// / half-dimensions) race test can assert the `embedding IS NOT NULL` filter
/// excludes mid-rebuild rows without exposing `SemanticHit`/`HalfVector`.
#[doc(hidden)]
#[allow(dead_code)] // pub test-only seam; consumed by tests/file_rag/mod.rs via the ziee::file_rag_search re-export
pub async fn vector_search_hit_count_for_test(
    scope_ids: &[Uuid],
    user_id: Uuid,
    query_vec: &[f32],
    threshold: f32,
    limit: i64,
) -> Result<usize, AppError> {
    let v = HalfVector::from_f32_slice(query_vec);
    Ok(vector_search(scope_ids, user_id, &v, threshold, limit).await?.len())
}

/// Test-only: number of FTS-arm hits. Re-exported alongside the vector wrapper.
#[doc(hidden)]
#[allow(dead_code)] // pub test-only seam; consumed by tests/file_rag/mod.rs via the ziee::file_rag_search re-export
pub async fn fts_search_hit_count_for_test(
    scope_ids: &[Uuid],
    user_id: Uuid,
    query: &str,
    limit: i64,
    dict: &str,
    min_rank: f32,
) -> Result<usize, AppError> {
    Ok(fts_search(scope_ids, user_id, query, limit, dict, min_rank).await?.len())
}

async fn vector_search(
    scope_ids: &[Uuid],
    user_id: Uuid,
    embedding: &HalfVector,
    threshold: f32,
    limit: i64,
) -> Result<Vec<SemanticHit>, AppError> {
    let rows = vector_rows(scope_ids, user_id, embedding, threshold, limit).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let score = 1.0 - r.metric;
            r.into_hit(score)
        })
        .collect())
}

async fn vector_rows(
    scope_ids: &[Uuid],
    user_id: Uuid,
    embedding: &HalfVector,
    threshold: f32,
    limit: i64,
) -> Result<Vec<ScoredChunkRow>, AppError> {
    let pool = Repos.file_rag.pool_clone();
    let sql = format!(
        "SELECT {SELECT_COLS}, (embedding <=> $2)::float8 AS metric \
         FROM file_chunks \
         WHERE file_id = ANY($1) AND user_id = $5 AND embedding IS NOT NULL AND (embedding <=> $2) < $3 \
         ORDER BY embedding <=> $2 LIMIT $4"
    );
    sqlx::query_as::<_, ScoredChunkRow>(&sql)
        .bind(scope_ids)
        .bind(embedding)
        .bind(threshold)
        .bind(limit)
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .map_err(AppError::database_error)
}

/// FTS (lexical) arm. Works with NO embedding model. `metric` = `ts_rank_cd`,
/// used directly as the hit score.
async fn fts_search(
    scope_ids: &[Uuid],
    user_id: Uuid,
    query: &str,
    limit: i64,
    dict: &str,
    min_rank: f32,
) -> Result<Vec<SemanticHit>, AppError> {
    let rows = fts_rows(scope_ids, user_id, query, limit, dict, min_rank).await?;
    Ok(rows.into_iter().map(|r| {
        let score = r.metric;
        r.into_hit(score)
    }).collect())
}

async fn fts_rows(
    scope_ids: &[Uuid],
    user_id: Uuid,
    query: &str,
    limit: i64,
    dict: &str,
    min_rank: f32,
) -> Result<Vec<ScoredChunkRow>, AppError> {
    let pool = Repos.file_rag.pool_clone();
    let sql = format!(
        "SELECT {SELECT_COLS}, \
            ts_rank_cd(content_tsv, websearch_to_tsquery($2::regconfig, $3))::float8 AS metric \
         FROM file_chunks \
         WHERE file_id = ANY($1) AND user_id = $6 \
           AND content_tsv @@ websearch_to_tsquery($2::regconfig, $3) \
           AND ts_rank_cd(content_tsv, websearch_to_tsquery($2::regconfig, $3)) >= $4 \
         ORDER BY ts_rank_cd(content_tsv, websearch_to_tsquery($2::regconfig, $3)) DESC LIMIT $5"
    );
    sqlx::query_as::<_, ScoredChunkRow>(&sql)
        .bind(scope_ids)
        .bind(dict)
        .bind(query)
        .bind(min_rank)
        .bind(limit)
        .bind(user_id)
        .fetch_all(&pool)
        .await
        .map_err(AppError::database_error)
}

/// Hybrid: pull a larger candidate pool from each arm, fuse with RRF.
#[allow(clippy::too_many_arguments)]
async fn hybrid_search(
    scope_ids: &[Uuid],
    user_id: Uuid,
    embedding: &HalfVector,
    threshold: f32,
    query: &str,
    limit: i64,
    dict: &str,
    min_rank: f32,
    rrf_k: i32,
    candidate_multiplier: i32,
) -> Result<Vec<SemanticHit>, AppError> {
    let candidate_k = (limit * candidate_multiplier as i64).max(limit);
    let vec_hits = vector_rows(scope_ids, user_id, embedding, threshold, candidate_k).await?;
    let fts_hits = fts_rows(scope_ids, user_id, query, candidate_k, dict, min_rank).await?;
    Ok(rrf_fuse(vec![vec_hits, fts_hits], rrf_k, limit as usize))
}

/// Reciprocal Rank Fusion over rank-ordered arms — rank-only, so the two
/// incomparable raw metrics never need normalizing. Deterministic tie-break
/// on chunk id so the `take(limit)` cutoff is stable run-to-run.
///
/// Standalone (mirrors `memory`'s inline formula verbatim) so the two never
/// silently diverge — the unit test below locks the formula.
fn rrf_fuse(arms: Vec<Vec<ScoredChunkRow>>, rrf_k: i32, limit: usize) -> Vec<SemanticHit> {
    let k = rrf_k as f64;
    let mut acc: HashMap<Uuid, (f64, ScoredChunkRow)> = HashMap::new();
    for arm in arms {
        for (rank, row) in arm.into_iter().enumerate() {
            let contrib = 1.0 / (k + (rank + 1) as f64);
            match acc.entry(row.id) {
                Entry::Occupied(mut e) => e.get_mut().0 += contrib,
                Entry::Vacant(e) => {
                    e.insert((contrib, row));
                }
            }
        }
    }
    let mut fused: Vec<(f64, ScoredChunkRow)> = acc.into_values().collect();
    fused.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.id.cmp(&b.1.id))
    });
    fused
        .into_iter()
        .take(limit)
        .map(|(score, row)| row.into_hit(score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: Uuid, content: &str) -> ScoredChunkRow {
        ScoredChunkRow {
            id,
            file_id: Uuid::nil(),
            blob_version_id: Uuid::nil(),
            version: 1,
            page_number: 1,
            char_start: 0,
            char_end: content.len() as i32,
            content: content.to_string(),
            metric: 0.0,
        }
    }

    #[test]
    fn rrf_rewards_appearing_in_both_arms() {
        // `both` is rank-2 in each arm; `va` is rank-1 in vector only; `fa`
        // rank-1 in fts only. Appearing in both should beat a single top-1.
        let va = Uuid::from_u128(1);
        let both = Uuid::from_u128(2);
        let fa = Uuid::from_u128(3);
        let vec_arm = vec![row(va, "va"), row(both, "both")];
        let fts_arm = vec![row(fa, "fa"), row(both, "both")];
        let fused = rrf_fuse(vec![vec_arm, fts_arm], 60, 10);
        assert_eq!(fused.len(), 3);
        // `both`: 1/(60+2) + 1/(60+2) ≈ 0.03226 > single top-1 1/(60+1) ≈ 0.01639.
        assert_eq!(
            fused[0].content, "both",
            "the chunk in both arms must rank first"
        );
    }

    #[test]
    fn rrf_is_deterministic_on_ties() {
        // Two chunks each appear once at rank-1 in one arm → equal scores;
        // tie-break by id ascending must be stable.
        let lo = Uuid::from_u128(10);
        let hi = Uuid::from_u128(20);
        let fused = rrf_fuse(vec![vec![row(hi, "hi")], vec![row(lo, "lo")]], 60, 10);
        assert_eq!(fused[0].content, "lo", "lower id wins the tie deterministically");
        assert_eq!(fused[1].content, "hi");
    }

    #[test]
    fn rrf_respects_limit() {
        let arm: Vec<ScoredChunkRow> = (0..5).map(|i| row(Uuid::from_u128(i), "x")).collect();
        let fused = rrf_fuse(vec![arm], 60, 3);
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn plan_arm_truth_table() {
        // has_vector × fts_enabled → the four retrieval arms.
        assert_eq!(plan_arm(true, true), Arm::Hybrid);
        assert_eq!(plan_arm(true, false), Arm::Vector);
        assert_eq!(plan_arm(false, true), Arm::Fts);
        assert_eq!(plan_arm(false, false), Arm::None);
    }

    /// Concurrent search during a re-embed (half-dimensions race) safety
    /// invariant: while `embed_worker::reembed_all` is rebuilding, the
    /// `ALTER COLUMN` NULLs every chunk's embedding before the new vectors
    /// land. A search firing in that window must NEVER surface a NULL-embedding
    /// row (which would be a half-dimension / dimensionless garbage hit) — the
    /// vector arm's `WHERE embedding IS NOT NULL` guard (see `vector_rows`)
    /// must drop it. This drives the REAL private `vector_rows` query against a
    /// chunk that has a valid 768-d embedding and a sibling chunk left NULL
    /// (exactly the mid-rebuild state) and asserts only the valid row returns.
    ///
    /// DB-gated: soft-skips (mirroring the suite's env-gated real-stack tests)
    /// when no Postgres is reachable, so `cargo test --lib` without a DB stays
    /// green; runs for real wherever `DATABASE_URL` points at a migrated DB.
    #[tokio::test]
    async fn vector_search_excludes_null_embeddings_during_rebuild() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise vector_rows against");
                return;
            }
        };
        let pool = match sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };
        // Idempotent: no-op if another lib test already initialized the factory.
        // Seed + query through the SAME factory pool `vector_rows` reads, so the
        // assertion holds regardless of which pool won the once-init race.
        crate::core::init_repositories(pool.clone());
        let db = Repos.file_rag.pool_clone();

        let tag = Uuid::new_v4();
        let user_id: Uuid =
            sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
                .bind(format!("rag_nullrace_{tag}"))
                .bind(format!("rag_nullrace_{tag}@example.com"))
                .fetch_one(&db)
                .await
                .expect("seed user");
        // `files.current_version_id` is NOT NULL (migration 93) with a DEFERRABLE
        // FK to `file_versions`. Seed both rows in one transaction (v1 id ==
        // file_id, the head-pointer invariant) so the deferred FK verifies at
        // COMMIT — a bare `INSERT INTO files` violates the NOT NULL.
        let file_id = Uuid::new_v4();
        {
            let mut tx = db.begin().await.expect("begin file seed tx");
            sqlx::query(
                "INSERT INTO files (id, user_id, filename, file_size, current_version_id) \
                 VALUES ($1, $2, 'rag.txt', 1, $1)",
            )
            .bind(file_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .expect("seed file");
            sqlx::query(
                "INSERT INTO file_versions \
                 (id, file_id, version, is_head, blob_version_id, file_size, created_by) \
                 VALUES ($1, $1, 1, true, $1, 1, 'user')",
            )
            .bind(file_id)
            .execute(&mut *tx)
            .await
            .expect("seed file_version");
            tx.commit().await.expect("commit file seed tx");
        }

        let embedding = HalfVector::from_f32_slice(&vec![0.1f32; 768]);
        // Chunk A: a fully-embedded row (post-rebuild / not yet NULLed).
        let valid_id: Uuid = sqlx::query_scalar(
            "INSERT INTO file_chunks \
             (file_id, user_id, blob_version_id, version, page_number, chunk_index, \
              char_start, char_end, content, embedding, embedding_model) \
             VALUES ($1, $2, $3, 1, 1, 0, 0, 5, 'hello', $4, 'test-model') RETURNING id",
        )
        .bind(file_id)
        .bind(user_id)
        .bind(Uuid::new_v4())
        .bind(&embedding)
        .fetch_one(&db)
        .await
        .expect("seed valid-embedding chunk");
        // Chunk B: embedding NULLed — the exact mid-`ALTER COLUMN` rebuild state.
        let null_id: Uuid = sqlx::query_scalar(
            "INSERT INTO file_chunks \
             (file_id, user_id, blob_version_id, version, page_number, chunk_index, \
              char_start, char_end, content, embedding) \
             VALUES ($1, $2, $3, 1, 1, 1, 0, 5, 'hello', NULL) RETURNING id",
        )
        .bind(file_id)
        .bind(user_id)
        .bind(Uuid::new_v4())
        .fetch_one(&db)
        .await
        .expect("seed NULL-embedding chunk");

        // threshold 2.5 > the cosine-distance ceiling of 2.0, so the only thing
        // that can drop the NULL row is the `IS NOT NULL` guard, not the metric.
        let query_vec = HalfVector::from_f32_slice(&vec![0.1f32; 768]);
        let rows = vector_rows(&[file_id], user_id, &query_vec, 2.5, 10)
            .await
            .expect("vector_rows must not error on a mid-rebuild corpus");

        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        assert!(
            ids.contains(&valid_id),
            "the valid-embedding chunk must be returned (got {ids:?})"
        );
        assert!(
            !ids.contains(&null_id),
            "a NULL-embedding chunk (mid-rebuild) must never surface in vector search (got {ids:?})"
        );

        // Cascade-clean the seeded user so the shared DB stays tidy.
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&db)
            .await;
    }
}
