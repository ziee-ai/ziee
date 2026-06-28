//! Background memory extraction — fires from `after_llm_call` via
//! `tokio::spawn` so the user-facing chat stream is never blocked.
//!
//! Pipeline:
//!   1. Gate checks (admin enabled + user extraction_enabled).
//!   2. Load the last user + last assistant message text.
//!   3. Load the user's 20 most-recent memories (for dedup bias).
//!   4. Call the extraction LLM (admin default or user override).
//!   5. Parse strict JSON; for each entry: ADD / UPDATE / DELETE / NOOP.
//!   6. ADD/UPDATE re-embed via the dispatcher and persist via repo.

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role};
use futures_util::StreamExt;
use pgvector::HalfVector;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::memory::models::is_valid_kind;
use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};

/// One extraction op emitted by the LLM.
#[derive(Debug, Deserialize)]
struct ExtractionOp {
    op: String,
    #[serde(default)]
    memory_id: Option<Uuid>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default = "default_importance_op")]
    importance: i16,
    #[serde(default = "default_confidence_op")]
    confidence: i16,
    #[serde(default = "default_kind_op")]
    kind: String,
}

fn default_importance_op() -> i16 {
    50
}
fn default_confidence_op() -> i16 {
    80
}
fn default_kind_op() -> String {
    "fact".to_string()
}

/// Entry point — called from `after_llm_call`'s spawned task.
pub async fn extract_and_persist(
    user_id: Uuid,
    user_message_text: String,
    assistant_message_text: String,
    source_message_id: Option<Uuid>,
) {
    if let Err(e) =
        run(user_id, user_message_text, assistant_message_text, source_message_id).await
    {
        tracing::warn!("memory.extract: pipeline error: {e}");
    }
}

async fn run(
    user_id: Uuid,
    user_message: String,
    assistant_message: String,
    source_message_id: Option<Uuid>,
) -> Result<(), AppError> {
    // ── 1. Gate ────────────────────────────────────────────────────
    let admin = Repos.memory.get_admin_settings().await?;
    if !admin.enabled {
        return Ok(());
    }
    let Some(embedding_model_id) = admin.embedding_model_id else {
        return Ok(());
    };

    let user_settings = Repos.memory.get_or_init_user_settings(user_id).await?;
    if !user_settings.extraction_enabled {
        return Ok(());
    }

    // Per-user daily extraction quota. Defaults to 200; admin can lift
    // by raising max_memories (also gates total live memory count).
    // Counts memories CREATED via extraction in the trailing 24h —
    // covers spam-via-many-short-conversations evasion since memories
    // accumulate globally per user, not per conversation. Plan §11.
    //
    // SOFT-CAP (audit R7-#3): the count-then-insert window means two
    // concurrent extractions can each see today_count = quota-1 and both
    // insert (total = quota+1). Acceptable — the quota is a brake against
    // casual spam, not a determined-attacker hard ceiling. The real
    // cost gate is the LLM API spend, not the row count. A hard-
    // enforce variant would need a BEFORE INSERT trigger or
    // SELECT FOR UPDATE NOWAIT; both add cost for marginal benefit.
    let daily_quota = i64::from(admin.daily_extraction_quota);
    let pool = Repos.memory.pool_clone();
    let today_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM user_memories
        WHERE user_id = $1
          AND source = 'extraction'
          AND created_at > NOW() - INTERVAL '24 hours'
        "#,
        user_id
    )
    .fetch_one(&pool)
    .await?;
    if today_count >= daily_quota {
        tracing::info!(
            "memory.extract: user {} hit daily extraction quota ({}/{}) — skipping",
            user_id,
            today_count,
            daily_quota
        );
        return Ok(());
    }

    // The extraction model can be the user's override or the admin default.
    let Some(extraction_model_id) = user_settings
        .extraction_model_id
        .or(admin.default_extraction_model_id)
    else {
        tracing::info!(
            "memory.extract: no extraction model configured (admin default or user override) — skipping"
        );
        return Ok(());
    };

    // Guard: the extraction model must be generation-capable. An
    // embedding model (text_embedding) is started with `--embeddings`
    // and returns HTTP 500 "the current context does not support logits
    // computation" on a chat request. Skip gracefully with a clear,
    // actionable log rather than firing a doomed request and swallowing
    // the 500. See `engine::capability`.
    let extraction_model = Repos
        .llm_model
        .get_by_id(extraction_model_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("LlmModel"))?;
    if let Some(reason) = super::capability::generation_unsupported_reason(
        &extraction_model.name,
        &extraction_model.capabilities,
    ) {
        tracing::warn!("memory.extract: {reason} — skipping extraction");
        return Ok(());
    }

    // ── 2. Load existing memories for dedup bias ───────────────────
    let existing = Repos
        .memory
        .list_for_user(user_id, 20, 0, None, None, None)
        .await?;
    let existing_block = if existing.is_empty() {
        "(no existing memories)".to_string()
    } else {
        existing
            .iter()
            .map(|m| format!("- {}  [id: {}]", m.content, m.id))
            .collect::<Vec<_>>()
            .join("\n")
    };

    // ── 3. Build extraction prompt ─────────────────────────────────
    let prompt = super::prompts::EXTRACTION_PROMPT
        .replace("{existing_memories_with_ids}", &existing_block)
        .replace("{user_message}", &user_message)
        .replace("{assistant_message}", &assistant_message);

    // ── 4. Call extraction LLM ─────────────────────────────────────
    let json_text = call_extraction_llm(&extraction_model, prompt).await?;

    // ── 5. Parse ops ───────────────────────────────────────────────
    let ops: Vec<ExtractionOp> = match parse_extraction_json(&json_text) {
        Some(o) => o,
        None => {
            tracing::warn!("memory.extract: extraction LLM returned malformed JSON; no writes");
            return Ok(());
        }
    };

    if ops.is_empty() {
        return Ok(());
    }

    let pool = Repos.memory.pool_clone();

    // Atomic quota enforcement (closes the count-then-insert TOCTOU flagged in
    // the pre-LLM soft-check above): take a per-user advisory lock on a
    // dedicated connection so concurrent extractions for the SAME user
    // serialize through the apply section, then RE-COUNT under the lock and
    // bail if the quota is now exhausted. Two racing extractions can no longer
    // both observe quota-1 and both insert. The lock is held only across the
    // apply loop (not the LLM call), and same-user extraction concurrency is
    // rare, so the serialization cost is negligible.
    let mut lock_conn = match pool.acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("memory.extract: could not acquire quota-lock connection: {e}");
            return Ok(());
        }
    };
    let lock_key = user_advisory_key(user_id);
    if let Err(e) = sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(lock_key)
        .execute(&mut *lock_conn)
        .await
    {
        tracing::warn!("memory.extract: quota advisory lock failed: {e}");
        return Ok(());
    }

    // Re-check the trailing-24h count now that we hold the lock.
    let recount = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM user_memories
        WHERE user_id = $1
          AND source = 'extraction'
          AND created_at > NOW() - INTERVAL '24 hours'
        "#,
        user_id
    )
    .fetch_one(&pool)
    .await;
    let over_quota = match recount {
        Ok(n) => n >= daily_quota,
        Err(e) => {
            tracing::warn!("memory.extract: quota recount failed: {e}");
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(lock_key)
                .execute(&mut *lock_conn)
                .await;
            return Ok(());
        }
    };
    if over_quota {
        tracing::info!(
            "memory.extract: user {} hit daily extraction quota under lock — skipping applies",
            user_id
        );
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(lock_key)
            .execute(&mut *lock_conn)
            .await;
        return Ok(());
    }

    // ── 6. Apply each op (under the per-user advisory lock) ─────────
    for op in ops {
        let outcome = match op.op.to_ascii_uppercase().as_str() {
            "NOOP" => Ok(()),
            "ADD" => apply_add(
                &pool,
                user_id,
                op.content,
                op.importance,
                op.kind,
                source_message_id,
                embedding_model_id,
            )
            .await,
            "UPDATE" => apply_update(
                &pool,
                user_id,
                op.memory_id,
                op.content,
                op.importance,
                op.kind,
                embedding_model_id,
            )
            .await,
            "DELETE" => apply_delete(user_id, op.memory_id).await,
            other => {
                tracing::warn!("memory.extract: unknown op {other:?}; ignoring");
                Ok(())
            }
        };
        if let Err(e) = outcome {
            tracing::warn!("memory.extract: op apply failed: {e}");
            // Continue with remaining ops — one bad op shouldn't kill
            // the rest of the batch.
        }
    }

    // Release the per-user quota lock. The apply loop above swallows per-op
    // errors (no early return), so this is always reached on the success path.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(lock_key)
        .execute(&mut *lock_conn)
        .await;

    Ok(())
}

/// Stable 64-bit key derived from a user UUID for `pg_advisory_lock`. XORed
/// with a fixed namespace constant so it won't collide with advisory keys a
/// future subsystem might derive the same way from a UUID.
fn user_advisory_key(user_id: Uuid) -> i64 {
    const NS: i64 = 0x6D_65_6D_5F_71_74_61_00; // "mem_qta\0"
    let b = user_id.as_bytes();
    let raw = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    raw ^ NS
}

async fn apply_add(
    pool: &PgPool,
    user_id: Uuid,
    content: Option<String>,
    importance: i16,
    kind: String,
    source_message_id: Option<Uuid>,
    embedding_model_id: Uuid,
) -> Result<(), AppError> {
    let content = match content {
        Some(c) if !c.trim().is_empty() => c,
        _ => return Ok(()),
    };
    // Clamp out-of-enum kinds to 'other' so the op degrades gracefully
    // instead of hitting the `user_memories.kind` CHECK and being dropped.
    let kind = if is_valid_kind(&kind) {
        kind
    } else {
        "other".to_string()
    };
    let new_row = Repos
        .memory
        .insert(
            user_id,
            &content,
            "extraction",
            importance.clamp(0, 100),
            &kind,
            &serde_json::json!({}),
            source_message_id,
            // Background extraction stays user-global (scope-aware extraction is
            // a documented future option; only explicit `remember` is scoped).
            "user",
            None,
            None,
        )
        .await?;

    // Embed + write back. The model NAME (not UUID) goes into
    // embedding_model so the re-embed worker can compare it cheaply
    // against the admin's currently-configured model name and skip
    // rows that don't need rebuilding.
    if let Ok(vec) = super::dispatch::embed(embedding_model_id, &content).await {
        let model_name = embedding_model_name(embedding_model_id).await;
        let v = HalfVector::from_f32_slice(&vec);
        let _ = sqlx::query(
            "UPDATE user_memories SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4",
        )
        .bind(&v)
        .bind(&model_name)
        .bind(new_row.id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }
    // Background path: notify the user's other devices to refresh the
    // memories list (origin None — not from a request connection).
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Create,
        new_row.id,
        Audience::owner(user_id),
        None,
    );
    Ok(())
}

/// Look up an llm_model's `name` for the `embedding_model` column.
/// Falls back to the UUID string on error.
async fn embedding_model_name(model_id: Uuid) -> String {
    match Repos.llm_model.get_by_id(model_id).await {
        Ok(Some(m)) => m.name,
        _ => model_id.to_string(),
    }
}

async fn apply_update(
    pool: &PgPool,
    user_id: Uuid,
    memory_id: Option<Uuid>,
    content: Option<String>,
    importance: i16,
    kind: String,
    embedding_model_id: Uuid,
) -> Result<(), AppError> {
    let Some(id) = memory_id else {
        return Ok(());
    };
    // Clamp out-of-enum kinds to 'other' (see apply_add) so a bad LLM-supplied
    // kind degrades the op instead of tripping the CHECK and dropping it.
    let kind = if is_valid_kind(&kind) {
        kind
    } else {
        "other".to_string()
    };
    let updated = Repos
        .memory
        .update_owned(
            user_id,
            id,
            content.as_deref(),
            Some(importance.clamp(0, 100)),
            Some(kind.as_str()),
            None,
        )
        .await?;
    let Some(row) = updated else {
        return Ok(());
    };

    if let Ok(vec) = super::dispatch::embed(embedding_model_id, &row.content).await {
        let model_name = embedding_model_name(embedding_model_id).await;
        let v = HalfVector::from_f32_slice(&vec);
        let _ = sqlx::query(
            "UPDATE user_memories SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4",
        )
        .bind(&v)
        .bind(&model_name)
        .bind(row.id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Update,
        row.id,
        Audience::owner(user_id),
        None,
    );
    Ok(())
}

async fn apply_delete(user_id: Uuid, memory_id: Option<Uuid>) -> Result<(), AppError> {
    let Some(id) = memory_id else {
        return Ok(());
    };
    let deleted = Repos.memory.soft_delete_owned(user_id, id).await?;
    if deleted {
        sync_publish(
            SyncEntity::Memory,
            SyncAction::Delete,
            id,
            Audience::owner(user_id),
            None,
        );
    }
    Ok(())
}

/// Call the extraction LLM. Single non-streaming completion accumulated
/// from the stream. The model is loaded + capability-checked by the
/// caller (`run`), so this takes the resolved `&LlmModel` directly.
async fn call_extraction_llm(
    model: &crate::modules::llm_model::models::LlmModel,
    prompt: String,
) -> Result<String, AppError> {
    let provider = Repos
        .llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::internal_error("Extraction provider not found"))?;

    let api_key = provider.api_key.as_deref().unwrap_or("");
    let base_url = provider.base_url.as_deref().ok_or_else(|| {
        AppError::internal_error(format!("Provider '{}' has no base_url", provider.name))
    })?;

    let ai_provider = Provider::new(&provider.provider_type, api_key, base_url)
        .map_err(|e| AppError::internal_error(format!("create extraction provider: {e}")))?;

    let request = ChatRequest {
        model: model.name.clone(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: prompt }],
        }],
        temperature: Some(0.2),
        max_tokens: Some(2048),
        ..Default::default()
    };

    let mut stream = ai_provider
        .chat_stream(request)
        .await
        .map_err(|e| AppError::internal_error(format!("extraction stream: {e}")))?;

    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::internal_error(format!("stream chunk: {e}")))?;
        for delta in &chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                buf.push_str(delta);
            }
        }
    }
    Ok(buf)
}

/// Parse the extraction LLM's response into ops. Tolerates wrapping
/// prose / markdown by extracting the first `[...]` JSON array.
fn parse_extraction_json(raw: &str) -> Option<Vec<ExtractionOp>> {
    let trimmed = raw.trim();
    // Strip ```json ... ``` fences if present.
    let stripped = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Locate the first array.
    let start = stripped.find('[')?;
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in stripped[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let array_text = &stripped[start..end];
    serde_json::from_str::<Vec<ExtractionOp>>(array_text).ok()
}
