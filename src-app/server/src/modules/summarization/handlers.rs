//! REST handlers for the summarization module.
//!
//!   `GET/PUT /api/summarization/settings`            — admin singleton
//!   `GET     /api/conversations/{id}/summary`        — owner-gated read
//!   `POST    /api/_test/summarization/refresh`       — debug-only

use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, extract::Path, http::StatusCode};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    core::{EventBus, Repos},
    modules::{
        permissions::{RequirePermissions, with_permission},
        summarization::{
            engine::summarizer::ConversationSummary,
            events::SummarizationEvent,
            models::{SummarizationAdminSettings, UpdateSummarizationAdminSettingsRequest},
            permissions::{SummarizationSettingsManage, SummarizationSettingsRead},
        },
        sync::{
            Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish,
        },
    },
};

// ─── admin settings ───────────────────────────────────────────────────

#[debug_handler]
pub async fn get_admin_settings(
    _auth: RequirePermissions<(SummarizationSettingsRead,)>,
) -> ApiResult<Json<SummarizationAdminSettings>> {
    let row = Repos.summarization.get_admin_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SummarizationSettingsRead,)>(op)
        .id("SummarizationAdmin.get")
        .tag("Summarization")
        .summary("Read deployment-wide summarization settings")
        .description(
            "Returns the singleton deployment-wide summarization settings \
             (enable flag, trigger thresholds, model, prompt).",
        )
        .response::<200, Json<SummarizationAdminSettings>>()
}

#[debug_handler]
pub async fn update_admin_settings(
    _auth: RequirePermissions<(SummarizationSettingsManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(body): Json<UpdateSummarizationAdminSettingsRequest>,
) -> ApiResult<Json<SummarizationAdminSettings>> {
    // Range validation in-handler so a bad value is a 400, not a raw
    // 500 from the DB CHECK. Mirrors the migration-91 CHECK constraints
    // (trigger 500..=1_000_000, keep >= 100, keep < trigger).
    if let Some(t) = body.summarize_after_tokens {
        if !(500..=1_000_000).contains(&t) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "summarize_after_tokens out of range (500..=1000000)",
            )
            .into());
        }
    }
    if let Some(k) = body.summarizer_keep_recent_tokens {
        if !(100..=1_000_000).contains(&k) {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "summarizer_keep_recent_tokens out of range (100..=1000000)",
            )
            .into());
        }
    }

    // Prompt-template validation: a non-empty override MUST contain
    // the placeholders the engine interpolates. Some(None) (clear back
    // to compiled default) and Some(Some("")) (clear-via-empty) both
    // skip validation — those reset to the compiled-in default which
    // is always valid.
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

    // Normalise empty-string → None so the engine's "NULL = compiled
    // default" fall-back keeps working when an admin clears a prompt
    // by deleting all the text instead of explicitly sending null.
    let full_summary_prompt = body
        .full_summary_prompt
        .map(|outer| outer.and_then(|s| if s.is_empty() { None } else { Some(s) }));
    let incremental_summary_prompt = body
        .incremental_summary_prompt
        .map(|outer| outer.and_then(|s| if s.is_empty() { None } else { Some(s) }));

    // FK pre-check: a bad `default_summarization_model_id` should be
    // a clean 400, not a raw 500 from the FK violation. Some(None)
    // (clear) and missing-field both bypass — only the explicit
    // Some(Some(id)) set case needs probing. Also enforce that the
    // picked model is `chat`-capable so the engine doesn't fail
    // silently at call time against an embedding-only / image-only
    // model (matches the FE dropdown's filter).
    if let Some(Some(model_id)) = body.default_summarization_model_id {
        match Repos.llm_model.get_by_id(model_id).await {
            Ok(Some(model)) => {
                if !model.capabilities.chat.unwrap_or(false) {
                    return Err(AppError::bad_request(
                        "VALIDATION_ERROR",
                        "default_summarization_model_id must reference a chat-capable model",
                    )
                    .into());
                }
            }
            Ok(None) => {
                return Err(AppError::bad_request(
                    "VALIDATION_ERROR",
                    "default_summarization_model_id refers to a non-existent llm_model",
                )
                .into());
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Effective keep < trigger invariant (the fields can be updated
    // independently, so check the merged values against the
    // migration-91 CHECK before the DB rejects it with a raw 500).
    let prior = Repos.summarization.get_admin_settings().await?;
    let effective_trigger = body
        .summarize_after_tokens
        .unwrap_or(prior.summarize_after_tokens);
    let effective_keep = body
        .summarizer_keep_recent_tokens
        .unwrap_or(prior.summarizer_keep_recent_tokens);
    if effective_keep >= effective_trigger {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "summarizer_keep_recent_tokens must be less than summarize_after_tokens",
        )
        .into());
    }

    let row = Repos
        .summarization
        .update_admin_settings(
            body.enabled,
            body.default_summarization_model_id,
            body.summarize_after_tokens,
            body.summarizer_keep_recent_tokens,
            full_summary_prompt,
            incremental_summary_prompt,
        )
        .await?;

    // Notify-only: the bus + sync entity carry just `{id}` so any handler
    // logging the event can't leak custom prompt content.
    event_bus.emit_async(SummarizationEvent::updated(Uuid::nil()).into());
    sync_publish(
        SyncEntity::SummarizationAdminSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<SummarizationSettingsRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(row)))
}

pub fn update_admin_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SummarizationSettingsManage,)>(op)
        .id("SummarizationAdmin.update")
        .tag("Summarization")
        .summary("Update deployment-wide summarization settings")
        .description(
            "Tri-state partial update — every field is optional. \
             Missing field = no change; explicit JSON null = clear back \
             to compiled default. Returns 400 on range / placeholder / \
             keep<trigger / unknown-llm-model violations before any DB write.",
        )
        .response::<200, Json<SummarizationAdminSettings>>()
        .response_with::<400, (), _>(|res| {
            res.description(
                "Validation failed (out-of-range token threshold, \
                 missing prompt placeholder, keep>=trigger, unknown \
                 default_summarization_model_id, or model that is not \
                 chat-capable).",
            )
        })
}

// ─── per-branch summary (owner-gated read) ───────────────────────────

#[debug_handler]
pub async fn get_conversation_summary(
    auth: RequirePermissions<(crate::modules::chat::core::permissions::ConversationsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<Option<ConversationSummary>>> {
    // Ownership: 404 (not 403) when the caller doesn't own the
    // conversation, to defeat probing for conversation ids.
    let conversation = Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    let Some(branch_id) = conversation.active_branch_id else {
        return Ok((StatusCode::OK, Json(None)));
    };

    let pool = Repos.summarization.pool_clone();
    let summary = crate::modules::summarization::engine::summarizer::fetch_summary(
        &pool, branch_id,
    )
    .await?;
    Ok((StatusCode::OK, Json(summary)))
}

pub fn get_conversation_summary_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(crate::modules::chat::core::permissions::ConversationsRead,)>(op)
        .id("Summarization.getConversationSummary")
        .tag("Summarization")
        .summary("Get the active-branch summary for a conversation (null if none)")
        .description(
            "Returns the rolling summary for the conversation's active branch, \
             or null when no summary has been generated yet.",
        )
        .response::<200, Json<Option<ConversationSummary>>>()
}

// ─── debug-only refresh hook ─────────────────────────────────────────

#[cfg(debug_assertions)]
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TestRefreshRequest {
    pub branch_id: Uuid,
    pub model_id: Uuid,
}

#[cfg(debug_assertions)]
#[debug_handler]
pub async fn test_refresh(
    _auth: RequirePermissions<(SummarizationSettingsManage,)>,
    Json(body): Json<TestRefreshRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::modules::summarization::engine::summarizer::refresh_summary(
        body.branch_id,
        body.model_id,
        // Manual refresh has no chat-model context → use the flat
        // admin threshold (no fraction-of-window override).
        None,
    )
    .await?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "ok": true }))))
}

#[cfg(debug_assertions)]
pub fn test_refresh_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SummarizationSettingsManage,)>(op)
        .id("SummarizationTest.refresh")
        .tag("Summarization")
        .summary("Test-only: trigger summary refresh synchronously (debug builds)")
        .description(
            "Debug-build-only endpoint that runs a synchronous summary refresh \
             for deterministic tests. Not present in release builds.",
        )
        .response::<200, Json<serde_json::Value>>()
}
