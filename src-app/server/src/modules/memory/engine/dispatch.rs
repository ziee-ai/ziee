//! Embedding dispatcher — routes embedding requests to the right
//! backend based on the configured embedding model's provider type.
//!
//! Two engines:
//! - `local` (provider_type = "local"): POST to the running
//!   `llama-server`'s `/embedding` endpoint (started with the
//!   `--embeddings` flag by `llm_local_runtime`).
//! - any remote provider with `embeddings()` support (openai, gemini,
//!   groq/etc. via openai-compat): call the `AIProvider::embeddings`
//!   trait method.
//!
//! The dispatcher loads the model row by id, looks up its provider,
//! and routes. No engine flag — provider type IS the engine.

use ai_providers::{EmbeddingsRequest, Provider};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;

/// One embedding call.
pub async fn embed(model_id: Uuid, text: &str) -> Result<Vec<f32>, AppError> {
    let vecs = embed_batch(model_id, &[text.to_string()]).await?;
    vecs.into_iter()
        .next()
        .ok_or_else(|| AppError::internal_error("embedding dispatcher returned no vectors"))
}

/// Batched embedding. Local engine batches via repeated calls (llama
/// server is single-input); remote providers may accept native arrays.
pub async fn embed_batch(
    model_id: Uuid,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, AppError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let model = Repos
        .llm_model
        .get_by_id(model_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("LlmModel"))?;

    if !model.capabilities.text_embedding.unwrap_or(false) {
        return Err(AppError::bad_request(
            "INVALID_EMBEDDING_MODEL",
            "configured model is not flagged with text_embedding capability",
        )
        .into());
    }

    let provider = Repos
        .llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::internal_error("Provider for embedding model not found"))?;

    // P1.g: chat extensions no longer branch on "local" — the proxy
    // architecture means a local provider's base_url + api_key are
    // OpenAI-compatible (injected by the llm_provider repository at
    // read time + minted on create). `embed_remote` works for both
    // remote AND local providers; the local engine is reached via
    // the proxy's `/v1/embeddings` endpoint with the PROXY_TOKEN.
    let vectors = embed_remote(&provider, &model, texts).await?;

    // Capability-tag honesty (plan §11). The model row claimed
    // `text_embedding=true` but the engine returned something that
    // isn't a usable embedding (empty / all-zero / NaN). Loudly log
    // so the admin sees memory-related health issues in server logs.
    // We don't mutate the row (that would race with admin edits) —
    // logging is sufficient and matches the audit-log surface
    // introduced by migration 50.
    for (i, v) in vectors.iter().enumerate() {
        if v.is_empty() {
            tracing::error!(
                "memory.dispatch: model {} ({}) returned an empty embedding for input #{} — \
                 the model is mis-tagged or not actually an embedder. Memory features will \
                 silently fail until the admin corrects capabilities.text_embedding.",
                model.id,
                model.name,
                i
            );
            return Err(AppError::internal_error(format!(
                "model '{}' is mis-tagged as text_embedding — engine returned empty vector",
                model.name
            )));
        }
        if v.iter().any(|f| !f.is_finite()) {
            tracing::error!(
                "memory.dispatch: model {} ({}) returned NaN/Inf in embedding for input #{} — \
                 likely a broken local engine instance.",
                model.id,
                model.name,
                i
            );
            return Err(AppError::internal_error(format!(
                "model '{}' returned a non-finite embedding component",
                model.name
            )));
        }
        if v.iter().all(|f| *f == 0.0) {
            tracing::warn!(
                "memory.dispatch: model {} ({}) returned an all-zero embedding — \
                 cosine similarity will be undefined; check engine readiness.",
                model.id,
                model.name
            );
        }
    }

    Ok(vectors)
}

// P1.g: `embed_local` was the dedicated local-engine path that
// bypassed the proxy. With the proxy architecture, local providers
// behave exactly like any OpenAI-compat provider — the chat path
// uses `embed_remote` for both. Removed; LlamaEmbeddingResponse +
// LlamaEmbeddingData were only consumed here, also removed.

/// Remote engine — delegate to the existing `AIProvider::embeddings`.
async fn embed_remote(
    provider: &crate::modules::llm_provider::models::LlmProvider,
    model: &crate::modules::llm_model::models::LlmModel,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, AppError> {
    let api_key = provider.api_key.as_deref().unwrap_or("");
    let base_url = provider
        .base_url
        .as_deref()
        .ok_or_else(|| {
            AppError::internal_error(format!(
                "Provider '{}' has no base_url configured",
                provider.name
            ))
        })?;

    let ai_provider = Provider::new(&provider.provider_type, api_key, base_url).map_err(|e| {
        AppError::internal_error(format!(
            "create embedding provider '{}': {e}",
            provider.provider_type
        ))
    })?;

    let request = EmbeddingsRequest {
        model: model.name.clone(),
        input: texts.to_vec(),
    };

    // Provider wrapper carries api_key + base_url internally — call site
    // passes only the request payload.
    let _ = (api_key, base_url);
    let resp = ai_provider
        .embeddings(request)
        .await
        .map_err(|e| AppError::internal_error(format!("provider embeddings error: {e}")))?;

    Ok(resp.embeddings)
}
