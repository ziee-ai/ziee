//! citations persistence: the bibliography library, the per-project link table,
//! and the idempotent built-in MCP server upsert.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{BibliographyEntry, VerificationStatus};

/// Fields needed to insert/merge a library entry. `csl_json` is the source of
/// truth; the scalars are its projection (see migration 102).
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub csl_json: Value,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub arxiv_id: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub dedup_fingerprint: Option<String>,
    pub citation_key: String,
    pub verification_status: VerificationStatus,
    pub source: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CitationsRepository {
    pool: PgPool,
}

impl CitationsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Idempotent upsert of the built-in citations MCP server row. Mirrors
    /// `web_search::upsert_builtin_server`.
    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        sqlx::query!(
            r#"
            INSERT INTO mcp_servers (
                id, user_id, name, display_name, description,
                enabled, is_system, is_built_in,
                transport_type, url, headers,
                timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                created_at, updated_at
            ) VALUES (
                $1, NULL, 'citations', 'Citations',
                'Built-in citation management + verification (lookup / add / verify / list / format)',
                true, true, true,
                'http', $2, '{}'::jsonb,
                30, false, 'auto', 4,
                NOW(), NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                is_system = EXCLUDED.is_system,
                is_built_in = EXCLUDED.is_built_in,
                transport_type = EXCLUDED.transport_type,
                url = EXCLUDED.url,
                updated_at = NOW()
            "#,
            server_id,
            loopback_url
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        tx.commit().await.map_err(AppError::database_error)?;
        Ok(())
    }

    /// Insert a new library entry, returning it. Dedup is decided by the engine
    /// before calling this (it looks up existing matches first); the partial
    /// unique indexes are the race backstop. A conflicting concurrent insert
    /// surfaces as `AppError::conflict` (409); the engine then either re-links to
    /// the existing row (DOI/PMID/fingerprint match) or regenerates the
    /// citation_key and retries (citation_key collision on a different work).
    pub async fn insert_entry(
        &self,
        user_id: Uuid,
        e: &NewEntry,
    ) -> Result<BibliographyEntry, AppError> {
        let row = sqlx::query!(
            r#"
            INSERT INTO bibliography_entries (
                user_id, csl_json, doi, pmid, pmcid, arxiv_id, title, year,
                dedup_fingerprint, citation_key, verification_status, verified_at, source
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                CASE WHEN $11 = 'verified' THEN NOW() ELSE NULL END, $12
            )
            RETURNING id, csl_json, doi, pmid, pmcid, arxiv_id, title, year,
                      citation_key, verification_status,
                      verified_at as "verified_at: chrono::DateTime<chrono::Utc>", source,
                      created_at as "created_at: chrono::DateTime<chrono::Utc>",
                      updated_at as "updated_at: chrono::DateTime<chrono::Utc>"
            "#,
            user_id,
            e.csl_json,
            e.doi,
            e.pmid,
            e.pmcid,
            e.arxiv_id,
            e.title,
            e.year,
            e.dedup_fingerprint,
            e.citation_key,
            e.verification_status.as_str(),
            e.source,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|err| match &err {
            // A concurrent insert of the same DOI/PMID/fingerprint tripped a
            // partial-unique index. Surface a typed conflict so the caller can
            // re-find the winner and link to it (dedup race).
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                AppError::conflict("bibliography entry")
            }
            _ => AppError::database_error(err),
        })?;

        Ok(BibliographyEntry {
            id: row.id,
            csl_json: row.csl_json,
            doi: row.doi,
            pmid: row.pmid,
            pmcid: row.pmcid,
            arxiv_id: row.arxiv_id,
            title: row.title,
            year: row.year,
            citation_key: row.citation_key,
            verification_status: VerificationStatus::from_db(&row.verification_status),
            verified_at: row.verified_at,
            source: row.source,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// One entry by id, scoped to the owner.
    pub async fn get_entry(
        &self,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<Option<BibliographyEntry>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT id, csl_json, doi, pmid, pmcid, arxiv_id, title, year,
                   citation_key, verification_status,
                   verified_at as "verified_at: chrono::DateTime<chrono::Utc>", source,
                   created_at as "created_at: chrono::DateTime<chrono::Utc>",
                   updated_at as "updated_at: chrono::DateTime<chrono::Utc>"
            FROM bibliography_entries
            WHERE user_id = $1 AND id = $2
            "#,
            user_id,
            id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(row.map(|row| BibliographyEntry {
            id: row.id,
            csl_json: row.csl_json,
            doi: row.doi,
            pmid: row.pmid,
            pmcid: row.pmcid,
            arxiv_id: row.arxiv_id,
            title: row.title,
            year: row.year,
            citation_key: row.citation_key,
            verification_status: VerificationStatus::from_db(&row.verification_status),
            verified_at: row.verified_at,
            source: row.source,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }

    /// The whole library, or (when `project_id` is set) only that project's
    /// reference list.
    pub async fn list_entries(
        &self,
        user_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<Vec<BibliographyEntry>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT b.id, b.csl_json, b.doi, b.pmid, b.pmcid, b.arxiv_id, b.title, b.year,
                   b.citation_key, b.verification_status,
                   b.verified_at as "verified_at: chrono::DateTime<chrono::Utc>", b.source,
                   b.created_at as "created_at: chrono::DateTime<chrono::Utc>",
                   b.updated_at as "updated_at: chrono::DateTime<chrono::Utc>"
            FROM bibliography_entries b
            WHERE b.user_id = $1
              AND ($2::uuid IS NULL OR EXISTS (
                    SELECT 1 FROM project_bibliography pb
                    WHERE pb.entry_id = b.id AND pb.project_id = $2
              ))
            ORDER BY b.created_at DESC
            "#,
            user_id,
            project_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(rows
            .into_iter()
            .map(|row| BibliographyEntry {
                id: row.id,
                csl_json: row.csl_json,
                doi: row.doi,
                pmid: row.pmid,
                pmcid: row.pmcid,
                arxiv_id: row.arxiv_id,
                title: row.title,
                year: row.year,
                citation_key: row.citation_key,
                verification_status: VerificationStatus::from_db(&row.verification_status),
                verified_at: row.verified_at,
                source: row.source,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect())
    }

    /// Update an entry's verification status (+ stamp verified_at on success).
    pub async fn set_verification(
        &self,
        user_id: Uuid,
        id: Uuid,
        status: VerificationStatus,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE bibliography_entries
            SET verification_status = $3,
                verified_at = CASE WHEN $3 = 'verified' THEN NOW() ELSE verified_at END,
                updated_at = NOW()
            WHERE user_id = $1 AND id = $2
            "#,
            user_id,
            id,
            status.as_str(),
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Delete an entry from the library (cascades its project links).
    pub async fn delete_entry(&self, user_id: Uuid, id: Uuid) -> Result<bool, AppError> {
        let res = sqlx::query!(
            r#"DELETE FROM bibliography_entries WHERE user_id = $1 AND id = $2"#,
            user_id,
            id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(res.rows_affected() > 0)
    }

    /// Link an entry into a project (idempotent; no-op if already linked).
    pub async fn attach_to_project(
        &self,
        project_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO project_bibliography (project_id, entry_id)
            VALUES ($1, $2)
            ON CONFLICT (project_id, entry_id) DO NOTHING
            "#,
            project_id,
            entry_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Unlink an entry from a project (the entry stays in the library).
    pub async fn detach_from_project(
        &self,
        project_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"DELETE FROM project_bibliography WHERE project_id = $1 AND entry_id = $2"#,
            project_id,
            entry_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// Find an existing entry by normalized DOI (dedup key 1).
    pub async fn find_by_doi(
        &self,
        user_id: Uuid,
        doi: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"SELECT id FROM bibliography_entries WHERE user_id = $1 AND lower(doi) = lower($2)"#,
            user_id,
            doi,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.id))
    }

    /// Find an existing entry by PMID (dedup key 2).
    pub async fn find_by_pmid(
        &self,
        user_id: Uuid,
        pmid: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"SELECT id FROM bibliography_entries WHERE user_id = $1 AND pmid = $2"#,
            user_id,
            pmid,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.id))
    }

    /// Identifier-less entries for the same year — the candidate block for a
    /// fuzzy "possible duplicate" title comparison (NOT auto-merged).
    pub async fn idless_candidates(
        &self,
        user_id: Uuid,
        year: Option<i32>,
    ) -> Result<Vec<(Uuid, Option<String>)>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, title FROM bibliography_entries
            WHERE user_id = $1 AND doi IS NULL AND pmid IS NULL
              AND year IS NOT DISTINCT FROM $2
            "#,
            user_id,
            year,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| (r.id, r.title)).collect())
    }

    /// Find an identifier-less entry by exact dedup fingerprint (dedup key 3).
    pub async fn find_by_fingerprint(
        &self,
        user_id: Uuid,
        fingerprint: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT id FROM bibliography_entries
            WHERE user_id = $1 AND doi IS NULL AND pmid IS NULL AND dedup_fingerprint = $2
            "#,
            user_id,
            fingerprint,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(|r| r.id))
    }

    /// Existing citation keys for a user (for collision-suffixing a new key).
    pub async fn existing_citation_keys(
        &self,
        user_id: Uuid,
        prefix: &str,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query!(
            r#"SELECT citation_key FROM bibliography_entries WHERE user_id = $1 AND citation_key LIKE $2"#,
            user_id,
            format!("{prefix}%"),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| r.citation_key).collect())
    }

}
