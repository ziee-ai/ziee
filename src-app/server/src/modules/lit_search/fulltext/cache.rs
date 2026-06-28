//! Full-text cache: a shared, content-addressed on-disk blob store + a Postgres
//! index (`lit_fulltext_cache`) + per-conversation hard-link "views" that the
//! sandbox mounts read-only at `/lit`.
//!
//! Layout under `<app_data>/lit-cache/`:
//!   * `blobs/<sha256>.txt`          — extracted full text (shared, deduped).
//!   * `views/<conversation_id>/…`   — hard links into blobs (readable names),
//!     the per-conversation set the sandbox mounts (no folder pollution).
//!
//! All content is public OA, so the blob store is deployment-wide.

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;

use super::resolvers::PaperIds;

/// Status persisted per cache entry (also the per-id status returned to the model).
pub const STATUS_FULL_TEXT: &str = "full_text";
pub const STATUS_NOT_OA: &str = "not_open_access";
pub const STATUS_NOT_FOUND: &str = "not_found";

/// Negative-result TTL (seconds). `full_text` rows never expire (OA content is
/// stable + content-addressed), but `not_open_access`/`not_found` rows are also
/// produced by TRANSIENT resolver failures (network/timeout/5xx all collapse to
/// these statuses). Without a TTL a single transient outage would permanently
/// poison a paper. `lookup` therefore ignores negative rows older than this and
/// re-resolves; `store`'s find-by-id UPDATE then refreshes the row in place.
const NEGATIVE_TTL_SECS: f64 = 6.0 * 3600.0;

pub fn cache_root() -> PathBuf {
    crate::core::get_app_data_dir().join("lit-cache")
}

fn blobs_dir() -> PathBuf {
    cache_root().join("blobs")
}

fn blob_path(hash: &str) -> PathBuf {
    blobs_dir().join(format!("{hash}.txt"))
}

fn view_dir(conversation_id: Uuid) -> PathBuf {
    cache_root().join("views").join(conversation_id.to_string())
}

/// The stable in-sandbox mount path for a conversation's full-text view.
pub const SANDBOX_MOUNT_PATH: &str = "/lit";

pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

/// A resolved cache row (the index entry; the blob is on disk at `content_hash`).
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub status: String,
    pub content_hash: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub version: Option<String>,
}

/// Look up a cached entry by ANY of the paper's ids (alias resolution). Touches
/// `last_accessed_at` for LRU on a hit.
pub async fn lookup(ids: &PaperIds) -> Result<Option<CacheEntry>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT id, content_hash, status, source, license, version
        FROM lit_fulltext_cache
        WHERE (($1::text IS NOT NULL AND doi = $1)
            OR ($2::text IS NOT NULL AND pmid = $2)
            OR ($3::text IS NOT NULL AND pmcid = $3)
            OR ($4::text IS NOT NULL AND arxiv_id = $4))
          -- ignore stale NEGATIVE rows (transient-failure poisoning) → re-resolve
          AND (status = 'full_text' OR fetched_at > NOW() - make_interval(secs => $5))
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
        ids.doi,
        ids.pmid,
        ids.pmcid,
        ids.arxiv_id,
        NEGATIVE_TTL_SECS,
    )
    .fetch_optional(Repos.pool())
    .await
    .map_err(AppError::database_error)?;

    let Some(row) = row else { return Ok(None) };
    // Touch LRU (best-effort).
    let _ = sqlx::query!(
        "UPDATE lit_fulltext_cache SET last_accessed_at = NOW() WHERE id = $1",
        row.id
    )
    .execute(Repos.pool())
    .await;

    Ok(Some(CacheEntry {
        status: row.status,
        content_hash: row.content_hash,
        source: row.source,
        license: row.license,
        version: row.version,
    }))
}

/// Read the extracted text for a cached blob, if present on disk.
pub fn read_blob(hash: &str) -> Option<String> {
    std::fs::read_to_string(blob_path(hash)).ok()
}

/// Store a resolved result: write the blob (when `text` is Some) and upsert the
/// index row keyed by the paper's ids. Returns the resulting cache entry.
pub async fn store(
    ids: &PaperIds,
    status: &str,
    text: Option<&str>,
    source: Option<&str>,
    license: Option<&str>,
    version: Option<&str>,
) -> Result<CacheEntry, AppError> {
    let (content_hash, byte_size) = match text {
        Some(t) => {
            let hash = sha256_hex(t);
            std::fs::create_dir_all(blobs_dir())
                .map_err(|e| AppError::internal_error(format!("lit-cache mkdir failed: {e}")))?;
            let path = blob_path(&hash);
            if !path.exists() {
                std::fs::write(&path, t)
                    .map_err(|e| AppError::internal_error(format!("lit-cache write failed: {e}")))?;
            }
            (Some(hash), t.len() as i64)
        }
        None => (None, 0),
    };

    // Find an existing row by ANY id (4 partial unique indexes mean a single
    // ON CONFLICT can't cover all of them), then UPDATE-or-INSERT. Two
    // concurrent first-fetches of the same id both SELECT None and both INSERT;
    // the loser hits a partial unique index. Retry ONCE on a unique-violation:
    // the re-find then sees the winner's row and UPDATEs it instead of erroring.
    for attempt in 0..2 {
        let existing: Option<i64> = sqlx::query_scalar!(
            r#"
            SELECT id FROM lit_fulltext_cache
            WHERE ($1::text IS NOT NULL AND doi = $1)
               OR ($2::text IS NOT NULL AND pmid = $2)
               OR ($3::text IS NOT NULL AND pmcid = $3)
               OR ($4::text IS NOT NULL AND arxiv_id = $4)
            LIMIT 1
            "#,
            ids.doi,
            ids.pmid,
            ids.pmcid,
            ids.arxiv_id,
        )
        .fetch_optional(Repos.pool())
        .await
        .map_err(AppError::database_error)?;

        if let Some(id) = existing {
            sqlx::query!(
                r#"
                UPDATE lit_fulltext_cache SET
                    doi = COALESCE($2, doi), pmid = COALESCE($3, pmid),
                    pmcid = COALESCE($4, pmcid), arxiv_id = COALESCE($5, arxiv_id),
                    content_hash = $6, status = $7, source = $8, license = $9, version = $10,
                    byte_size = $11, fetched_at = NOW(), last_accessed_at = NOW()
                WHERE id = $1
                "#,
                id,
                ids.doi,
                ids.pmid,
                ids.pmcid,
                ids.arxiv_id,
                content_hash,
                status,
                source,
                license,
                version,
                byte_size,
            )
            .execute(Repos.pool())
            .await
            .map_err(AppError::database_error)?;
            break;
        }

        let insert = sqlx::query!(
            r#"
            INSERT INTO lit_fulltext_cache
                (doi, pmid, pmcid, arxiv_id, content_hash, status, source, license, version, byte_size, fetched_at, last_accessed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
            "#,
            ids.doi,
            ids.pmid,
            ids.pmcid,
            ids.arxiv_id,
            content_hash,
            status,
            source,
            license,
            version,
            byte_size,
        )
        .execute(Repos.pool())
        .await;
        match insert {
            Ok(_) => break,
            // Lost the insert race — loop once more; the re-find finds the row.
            Err(e)
                if attempt == 0
                    && e.as_database_error().is_some_and(|d| d.is_unique_violation()) =>
            {
                continue;
            }
            Err(e) => return Err(AppError::database_error(e)),
        }
    }

    Ok(CacheEntry {
        status: status.to_string(),
        content_hash,
        source: source.map(String::from),
        license: license.map(String::from),
        version: version.map(String::from),
    })
}

/// Hard-link a cached blob into a conversation's view dir under a readable name.
/// Hard links (not symlinks) so the file survives even if the blob is later
/// LRU-evicted while a sandbox session holds the view, and work cross-platform.
pub fn link_into_view(
    conversation_id: Uuid,
    hash: &str,
    display_name: &str,
) -> Result<String, AppError> {
    let dir = view_dir(conversation_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(format!("lit-cache view mkdir failed: {e}")))?;
    let filename = format!("{}.txt", sanitize(display_name));
    let dest = dir.join(&filename);
    if dest.exists() {
        return Ok(filename);
    }
    match std::fs::hard_link(blob_path(hash), &dest) {
        Ok(()) => Ok(filename),
        // Fall back to a copy if hard-linking is unsupported (cross-device).
        Err(_) => match read_blob(hash) {
            Some(text) => {
                std::fs::write(&dest, text)
                    .map_err(|e| AppError::internal_error(format!("lit-cache view write failed: {e}")))?;
                Ok(filename)
            }
            None => Ok(filename),
        },
    }
}

/// The on-disk view dir for a conversation (what the sandbox bind-mounts at /lit).
pub fn conversation_view_dir(conversation_id: Uuid) -> PathBuf {
    view_dir(conversation_id)
}

/// Best-effort removal of a conversation's full-text view dir (the per-
/// conversation `/lit` mount source full of hard-linked blobs). Call this when
/// the conversation is deleted so these dirs don't accumulate on disk forever.
/// A missing dir is a no-op; failures are logged, never propagated (cleanup
/// must not fail the delete).
pub fn cleanup_conversation_view(conversation_id: Uuid) {
    let dir = view_dir(conversation_id);
    if !dir.exists() {
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(&dir) {
        tracing::warn!(
            conversation_id = %conversation_id,
            error = %e,
            "lit-cache: failed to remove conversation view dir on delete"
        );
    }
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '.' || c == '_' { c } else { '_' })
        .collect();
    let s = s.trim_matches('_');
    if s.is_empty() { "paper".to_string() } else { s.chars().take(120).collect() }
}

/// Size-bounded LRU eviction: delete the least-recently-accessed entries (and
/// their blobs) beyond `keep`. Best-effort; pinned-by-view blobs survive because
/// the view holds an independent hard link / copy.
pub async fn evict_lru(keep: i64) -> Result<u64, AppError> {
    let stale = sqlx::query!(
        r#"
        SELECT id, content_hash FROM lit_fulltext_cache
        WHERE id NOT IN (
            SELECT id FROM lit_fulltext_cache ORDER BY last_accessed_at DESC LIMIT $1
        )
        "#,
        keep
    )
    .fetch_all(Repos.pool())
    .await
    .map_err(AppError::database_error)?;

    // Delete the index rows FIRST, collecting their (deduped) blob hashes.
    let mut removed = 0u64;
    let mut hashes: Vec<String> = Vec::new();
    for row in stale {
        if let Some(hash) = row.content_hash {
            hashes.push(hash);
        }
        let _ = sqlx::query!("DELETE FROM lit_fulltext_cache WHERE id = $1", row.id)
            .execute(Repos.pool())
            .await;
        removed += 1;
    }
    // Blobs are content-addressed + deduped, so two rows can share one file.
    // Only unlink a blob once NO surviving row still references its hash.
    hashes.sort();
    hashes.dedup();
    for hash in hashes {
        let still_referenced = sqlx::query_scalar!(
            "SELECT 1 FROM lit_fulltext_cache WHERE content_hash = $1 LIMIT 1",
            hash
        )
        .fetch_optional(Repos.pool())
        .await
        .map_err(AppError::database_error)?
        .is_some();
        if !still_referenced {
            let _ = std::fs::remove_file(blob_path(&hash));
        }
    }
    Ok(removed)
}
