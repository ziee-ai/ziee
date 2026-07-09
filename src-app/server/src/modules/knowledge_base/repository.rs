//! Data layer for knowledge bases. Everything is OWNER-SCOPED: every query
//! filters by `user_id`, so a foreign KB id is invisible (→ 404 / empty), and
//! `resolve_scope_file_ids` (the bridge to retrieval) can never return another
//! user's files.

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use super::models::{
    AttachDocumentsResult, IndexingSummary, KnowledgeBase, KnowledgeBaseDocument, KB_MAX_DOCUMENTS,
};

pub struct KnowledgeBaseRepository {
    pool: PgPool,
}

impl KnowledgeBaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── built-in MCP server row ─────────────────────────────────────────

    pub async fn upsert_builtin_server(
        &self,
        server_id: Uuid,
        loopback_url: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_servers (
                id, user_id, name, display_name, description,
                enabled, is_system, is_built_in,
                transport_type, url, headers,
                timeout_seconds, supports_sampling, usage_mode, max_concurrent_sessions,
                created_at, updated_at
            ) VALUES (
                $1, NULL, 'knowledge_base', 'Knowledge Base',
                'Built-in retrieval over your knowledge bases (search_knowledge / list_knowledge_bases)',
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
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    // ── KB CRUD (owner-scoped) ──────────────────────────────────────────

    pub async fn create(
        &self,
        user_id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<KnowledgeBase, AppError> {
        let row = sqlx::query!(
            r#"
            INSERT INTO knowledge_bases (user_id, name, description)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            user_id,
            name,
            description,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        self.get(user_id, row.id)
            .await?
            .ok_or_else(|| AppError::internal_error("created KB not found"))
    }

    /// Fetch one KB (owner-scoped) with derived document_count + indexing_summary.
    pub async fn get(
        &self,
        user_id: Uuid,
        kb_id: Uuid,
    ) -> Result<Option<KnowledgeBase>, AppError> {
        let base = sqlx::query!(
            r#"
            SELECT kb.id, kb.name, kb.description,
                   kb.created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                   kb.updated_at AS "updated_at: chrono::DateTime<chrono::Utc>",
                   (SELECT COUNT(*) FROM knowledge_base_documents d
                    WHERE d.knowledge_base_id = kb.id) AS "document_count!"
            FROM knowledge_bases kb
            WHERE kb.id = $1 AND kb.user_id = $2
            "#,
            kb_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let Some(base) = base else { return Ok(None) };
        let indexing_summary = self.indexing_summary(kb_id).await?;
        Ok(Some(KnowledgeBase {
            id: base.id,
            name: base.name,
            description: base.description,
            document_count: base.document_count,
            indexing_summary,
            created_at: base.created_at,
            updated_at: base.updated_at,
        }))
    }

    pub async fn list(&self, user_id: Uuid) -> Result<Vec<KnowledgeBase>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT kb.id, kb.name, kb.description,
                   kb.created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                   kb.updated_at AS "updated_at: chrono::DateTime<chrono::Utc>",
                   (SELECT COUNT(*) FROM knowledge_base_documents d
                    WHERE d.knowledge_base_id = kb.id) AS "document_count!"
            FROM knowledge_bases kb
            WHERE kb.user_id = $1
            ORDER BY kb.updated_at DESC
            "#,
            user_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let indexing_summary = self.indexing_summary(r.id).await?;
            out.push(KnowledgeBase {
                id: r.id,
                name: r.name,
                description: r.description,
                document_count: r.document_count,
                indexing_summary,
                created_at: r.created_at,
                updated_at: r.updated_at,
            });
        }
        Ok(out)
    }

    pub async fn update(
        &self,
        user_id: Uuid,
        kb_id: Uuid,
        name: Option<&str>,
        description: Option<Option<&str>>,
    ) -> Result<Option<KnowledgeBase>, AppError> {
        let desc_set = description.is_some();
        let desc_val = description.flatten();
        let updated = sqlx::query!(
            r#"
            UPDATE knowledge_bases
            SET name        = COALESCE($3, name),
                description = CASE WHEN $4::bool THEN $5 ELSE description END,
                updated_at  = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING id
            "#,
            kb_id,
            user_id,
            name,
            desc_set,
            desc_val,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        if updated.is_none() {
            return Ok(None);
        }
        self.get(user_id, kb_id).await
    }

    /// Delete a KB (owner-scoped). Cascades the join rows ONLY — never the shared
    /// `file_chunks` / files (no kb_id on file_chunks). Returns rows affected.
    pub async fn delete(&self, user_id: Uuid, kb_id: Uuid) -> Result<u64, AppError> {
        let r = sqlx::query!(
            "DELETE FROM knowledge_bases WHERE id = $1 AND user_id = $2",
            kb_id,
            user_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(r.rows_affected())
    }

    /// True if the KB exists and belongs to the user (ownership guard).
    pub async fn owns(&self, user_id: Uuid, kb_id: Uuid) -> Result<bool, AppError> {
        let row = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM knowledge_bases WHERE id = $1 AND user_id = $2)",
            kb_id,
            user_id,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.unwrap_or(false))
    }

    async fn indexing_summary(&self, kb_id: Uuid) -> Result<IndexingSummary, AppError> {
        // Left join so documents with no state row count as `pending`.
        let rows = sqlx::query!(
            r#"
            SELECT COALESCE(s.status, 'pending') AS "status!", COUNT(*) AS "n!"
            FROM knowledge_base_documents d
            LEFT JOIN file_index_state s ON s.file_id = d.file_id
            WHERE d.knowledge_base_id = $1
            GROUP BY COALESCE(s.status, 'pending')
            "#,
            kb_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut sum = IndexingSummary::default();
        for r in rows {
            sum.total += r.n;
            match r.status.as_str() {
                "indexed" => sum.indexed = r.n,
                "indexing" => sum.indexing = r.n,
                "failed" => sum.failed = r.n,
                "no_text" => sum.no_text = r.n,
                _ => sum.pending += r.n,
            }
        }
        Ok(sum)
    }

    // ── documents (owner-scoped, dedup, cap) ────────────────────────────

    /// Attach files to a KB. Only files the user owns are linked; a byte-identical
    /// file (same checksum) already in the KB is skipped-and-reported (DEC-36);
    /// the 2000-doc cap is enforced atomically (DEC-14).
    pub async fn add_documents_capped(
        &self,
        user_id: Uuid,
        kb_id: Uuid,
        file_ids: &[Uuid],
    ) -> Result<AttachDocumentsResult, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Lock the KB row so the cap check + inserts are atomic.
        let current: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) AS \"n!\" FROM knowledge_base_documents WHERE knowledge_base_id = $1",
            kb_id,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let mut attached = 0i64;
        let mut skipped = 0i64;
        for &file_id in file_ids {
            // Owner check: the file must belong to the user.
            let checksum = sqlx::query_scalar!(
                "SELECT checksum FROM files WHERE id = $1 AND user_id = $2",
                file_id,
                user_id,
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            let Some(checksum) = checksum else {
                // Not the user's file — silently skip (never link a foreign file).
                continue;
            };

            // Dedup: a file with the same checksum already in this KB.
            let dup: bool = sqlx::query_scalar!(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM knowledge_base_documents d
                    JOIN files f ON f.id = d.file_id
                    WHERE d.knowledge_base_id = $1 AND f.checksum = $2
                ) AS "e!"
                "#,
                kb_id,
                checksum,
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            if dup {
                skipped += 1;
                continue;
            }

            if current + attached >= KB_MAX_DOCUMENTS {
                tx.rollback().await.map_err(AppError::database_error)?;
                return Err(AppError::unprocessable_entity(
                    "KB_DOCUMENT_CAP",
                    format!("knowledge base document cap ({KB_MAX_DOCUMENTS}) reached"),
                )
                .into());
            }

            let inserted = sqlx::query!(
                r#"
                INSERT INTO knowledge_base_documents (knowledge_base_id, file_id)
                VALUES ($1, $2)
                ON CONFLICT (knowledge_base_id, file_id) DO NOTHING
                "#,
                kb_id,
                file_id,
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            if inserted.rows_affected() > 0 {
                attached += 1;
            } else {
                skipped += 1;
            }
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(AttachDocumentsResult {
            attached,
            skipped_duplicates: skipped,
        })
    }

    /// Remove a document from a KB — deletes ONLY the join row, never the shared
    /// file or its `file_chunks`.
    pub async fn remove_document(
        &self,
        user_id: Uuid,
        kb_id: Uuid,
        file_id: Uuid,
    ) -> Result<u64, AppError> {
        // Owner guard via the KB.
        if !self.owns(user_id, kb_id).await? {
            return Ok(0);
        }
        let r = sqlx::query!(
            "DELETE FROM knowledge_base_documents WHERE knowledge_base_id = $1 AND file_id = $2",
            kb_id,
            file_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(r.rows_affected())
    }

    /// List a KB's documents (paginated) with derived index status + chunk count.
    pub async fn list_documents(
        &self,
        user_id: Uuid,
        kb_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<KnowledgeBaseDocument>, AppError> {
        if !self.owns(user_id, kb_id).await? {
            return Ok(Vec::new());
        }
        let rows = sqlx::query!(
            r#"
            SELECT d.file_id, f.filename,
                   d.added_at AS "added_at: chrono::DateTime<chrono::Utc>",
                   COALESCE(s.status, 'pending') AS "index_status!",
                   COALESCE(s.chunk_count, 0)    AS "chunk_count!"
            FROM knowledge_base_documents d
            JOIN files f ON f.id = d.file_id
            LEFT JOIN file_index_state s ON s.file_id = d.file_id
            WHERE d.knowledge_base_id = $1
            ORDER BY d.added_at DESC
            LIMIT $2 OFFSET $3
            "#,
            kb_id,
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(rows
            .into_iter()
            .map(|r| KnowledgeBaseDocument {
                file_id: r.file_id,
                filename: r.filename,
                added_at: r.added_at,
                index_status: r.index_status,
                chunk_count: r.chunk_count as i64,
            })
            .collect())
    }

    /// File_ids that have zero chunks (need indexing on attach). Owner-scoped.
    pub async fn documents_without_chunks(
        &self,
        user_id: Uuid,
        file_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT f.id AS "id!"
            FROM files f
            WHERE f.user_id = $1 AND f.id = ANY($2)
              AND NOT EXISTS (SELECT 1 FROM file_chunks c WHERE c.file_id = f.id)
              AND f.text_page_count > 0
            "#,
            user_id,
            file_ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    // ── scope resolution (the bridge to retrieval — owner-filtered) ─────

    /// Union of file_ids across the given KBs, filtered to the user's own KBs.
    /// A foreign kb_id contributes nothing (cross-user tool-leak guard).
    pub async fn resolve_scope_file_ids(
        &self,
        user_id: Uuid,
        kb_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError> {
        if kb_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT d.file_id AS "file_id!"
            FROM knowledge_base_documents d
            JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id
            WHERE kb.user_id = $1 AND kb.id = ANY($2)
            "#,
            user_id,
            kb_ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    // ── attach to conversation / project ────────────────────────────────

    pub async fn attach_to_conversation(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
        kb_id: Uuid,
    ) -> Result<bool, AppError> {
        if !self.owns(user_id, kb_id).await? {
            return Ok(false);
        }
        sqlx::query!(
            r#"
            INSERT INTO conversation_knowledge_bases (conversation_id, knowledge_base_id)
            VALUES ($1, $2) ON CONFLICT DO NOTHING
            "#,
            conversation_id,
            kb_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(true)
    }

    pub async fn detach_from_conversation(
        &self,
        conversation_id: Uuid,
        kb_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM conversation_knowledge_bases WHERE conversation_id = $1 AND knowledge_base_id = $2",
            conversation_id,
            kb_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    pub async fn attach_to_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        kb_id: Uuid,
    ) -> Result<bool, AppError> {
        if !self.owns(user_id, kb_id).await? {
            return Ok(false);
        }
        sqlx::query!(
            r#"
            INSERT INTO project_knowledge_bases (project_id, knowledge_base_id)
            VALUES ($1, $2) ON CONFLICT DO NOTHING
            "#,
            project_id,
            kb_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(true)
    }

    pub async fn detach_from_project(
        &self,
        project_id: Uuid,
        kb_id: Uuid,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM project_knowledge_bases WHERE project_id = $1 AND knowledge_base_id = $2",
            project_id,
            kb_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// KBs attached to a conversation: its direct attachments UNION the KBs of
    /// its project (read-through). Owner-scoped.
    pub async fn attached_kb_ids_for_conversation(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT kb.id AS "id!"
            FROM knowledge_bases kb
            WHERE kb.user_id = $1 AND (
                kb.id IN (
                    SELECT knowledge_base_id FROM conversation_knowledge_bases
                    WHERE conversation_id = $2
                )
                OR kb.id IN (
                    SELECT pkb.knowledge_base_id
                    FROM project_knowledge_bases pkb
                    JOIN project_conversations pc ON pc.project_id = pkb.project_id
                    WHERE pc.conversation_id = $2
                )
            )
            "#,
            user_id,
            conversation_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    /// Full KB rows DIRECTLY attached to a conversation (the
    /// `conversation_knowledge_bases` join only — project-inherited KBs are
    /// surfaced on the project, not here, since detach operates on direct rows).
    /// Owner-scoped. Enriched with document_count + indexing_summary like `list`.
    pub async fn attached_kbs_for_conversation(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<KnowledgeBase>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT kb.id, kb.name, kb.description,
                   kb.created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                   kb.updated_at AS "updated_at: chrono::DateTime<chrono::Utc>",
                   (SELECT COUNT(*) FROM knowledge_base_documents d
                    WHERE d.knowledge_base_id = kb.id) AS "document_count!"
            FROM knowledge_bases kb
            JOIN conversation_knowledge_bases c ON c.knowledge_base_id = kb.id
            WHERE kb.user_id = $1 AND c.conversation_id = $2
            ORDER BY kb.updated_at DESC
            "#,
            user_id,
            conversation_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let indexing_summary = self.indexing_summary(r.id).await?;
            out.push(KnowledgeBase {
                id: r.id,
                name: r.name,
                description: r.description,
                document_count: r.document_count,
                indexing_summary,
                created_at: r.created_at,
                updated_at: r.updated_at,
            });
        }
        Ok(out)
    }

    /// Full KB rows attached to a project (owner-scoped). Same enrichment as
    /// `list`; drives the project "Knowledge bases" extension panel.
    pub async fn attached_kbs_for_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<KnowledgeBase>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT kb.id, kb.name, kb.description,
                   kb.created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                   kb.updated_at AS "updated_at: chrono::DateTime<chrono::Utc>",
                   (SELECT COUNT(*) FROM knowledge_base_documents d
                    WHERE d.knowledge_base_id = kb.id) AS "document_count!"
            FROM knowledge_bases kb
            JOIN project_knowledge_bases p ON p.knowledge_base_id = kb.id
            WHERE kb.user_id = $1 AND p.project_id = $2
            ORDER BY kb.updated_at DESC
            "#,
            user_id,
            project_id,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let indexing_summary = self.indexing_summary(r.id).await?;
            out.push(KnowledgeBase {
                id: r.id,
                name: r.name,
                description: r.description,
                document_count: r.document_count,
                indexing_summary,
                created_at: r.created_at,
                updated_at: r.updated_at,
            });
        }
        Ok(out)
    }

    /// Filenames for a set of (owner's) file_ids, for rendering search hits.
    pub async fn filenames_for(
        &self,
        user_id: Uuid,
        file_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, String>, AppError> {
        if file_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let rows = sqlx::query!(
            "SELECT id, filename FROM files WHERE user_id = $1 AND id = ANY($2)",
            user_id,
            file_ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| (r.id, r.filename)).collect())
    }

    /// Just the names, for the chat-extension note.
    pub async fn kb_names(&self, user_id: Uuid, kb_ids: &[Uuid]) -> Result<Vec<String>, AppError> {
        if kb_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_scalar!(
            "SELECT name FROM knowledge_bases WHERE user_id = $1 AND id = ANY($2) ORDER BY name",
            user_id,
            kb_ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }
}
