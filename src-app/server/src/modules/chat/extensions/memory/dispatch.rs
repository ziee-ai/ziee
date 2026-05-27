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
use serde::Deserialize;
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

    let vectors = match provider.provider_type.as_str() {
        "local" => embed_local(&model, texts).await?,
        _ => embed_remote(&provider, &model, texts).await?,
    };

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

/// Local engine — POST to the running llama-server `/embedding` route.
/// Expects the runtime to have started the model with `--embeddings`.
async fn embed_local(
    model: &crate::modules::llm_model::models::LlmModel,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, AppError> {
    let port = model.port.ok_or_else(|| {
        AppError::internal_error(
            "local embedding model has no port — instance not started (start the model from the LLM Models page)",
        )
    })?;

    let api_key =
        crate::modules::llm_local_runtime::deployment::local::get_instance_api_key(model.id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::internal_error(format!("reqwest build: {e}")))?;

    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let mut req = client
            .post(format!("http://127.0.0.1:{port}/embedding"))
            .json(&serde_json::json!({ "content": text }));
        if let Some(ref k) = api_key {
            req = req.bearer_auth(k);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::internal_error(format!("llama-server /embedding: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::internal_error(format!(
                "llama-server /embedding returned {status}: {body}"
            ))
            .into());
        }

        let body: LlamaEmbeddingResponse = resp
            .json()
            .await
            .map_err(|e| AppError::internal_error(format!("parse embedding json: {e}")))?;

        let vec = body
            .embedding
            .or_else(|| body.data.into_iter().next().and_then(|d| d.embedding))
            .ok_or_else(|| {
                AppError::internal_error("llama-server /embedding: no embedding in response")
            })?;
        out.push(vec);
    }
    Ok(out)
}

/// Llama-server `/embedding` response. Supports both the legacy
/// `{"embedding": [...]}` shape and the OpenAI-compat `data` shape.
#[derive(Debug, Deserialize)]
struct LlamaEmbeddingResponse {
    #[serde(default)]
    embedding: Option<Vec<f32>>,
    #[serde(default)]
    data: Vec<LlamaEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct LlamaEmbeddingData {
    #[serde(default)]
    embedding: Option<Vec<f32>>,
}

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
