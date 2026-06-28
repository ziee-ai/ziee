//! `GET /api/llm-providers/{id}/discover-models`
//!
//! P1.j of feat/local-llm-runtime. Per-provider model discovery
//! with three layers:
//!  1. Catalog hit (`known_models.json`)
//!  2. Live `/v1/models` fetch (Gemini + Groq expose structured
//!     capabilities; others list IDs only)
//!  3. Operator override (drawer field)
//!
//! Local providers are handled separately — they call the proxy's
//! `/v1/models` endpoint, which lists models from the server's own
//! DB. So this endpoint short-circuits with a friendly error for
//! local providers.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Path,
    http::StatusCode,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::r#type::{ApiResult, AppError},
    core::repository::Repos,
    modules::permissions::{RequirePermissions, with_permission},
};

use super::super::permissions::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveredModel {
    pub id: String,
    pub display_name: Option<String>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_chat: bool,
    pub supports_embeddings: bool,
    pub supports_vision: bool,
    pub supports_tool_use: Option<bool>,
    pub deprecated: bool,
    /// "catalog" | "discovery" | "operator_override"
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoverModelsResponse {
    pub provider_type: String,
    pub models: Vec<DiscoveredModel>,
    pub notes: Vec<String>,
}

#[debug_handler]
pub async fn discover_models(
    _auth: RequirePermissions<(LlmProvidersRead,)>,
    Path(provider_id): Path<Uuid>,
) -> ApiResult<Json<DiscoverModelsResponse>> {
    let provider = Repos
        .llm_provider
        .get_by_id(provider_id)
        .await
        .map_err(|e| {
            tracing::error!("discover_models: get provider: {e}");
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    let provider_type = provider.provider_type.clone();
    let mut notes = Vec::new();
    let mut models: Vec<DiscoveredModel> = Vec::new();

    if provider_type == "local" {
        notes.push(
            "Local providers list their own models from the database; use \
             /api/llm-models?provider_id=... instead."
                .to_string(),
        );
        return Ok((
            StatusCode::OK,
            Json(DiscoverModelsResponse {
                provider_type,
                models,
                notes,
            }),
        ));
    }

    // Layer 1: curated catalog.
    let catalog_ids = ai_providers::registry_known_ids(&provider_type);
    for id in &catalog_ids {
        if let Some(c) = ai_providers::registry_lookup(&provider_type, id) {
            models.push(DiscoveredModel {
                id: id.clone(),
                display_name: c.display_name,
                context_length: c.context_length,
                max_output_tokens: c.max_output_tokens,
                supports_chat: c.supports_chat,
                supports_embeddings: c.supports_embeddings,
                supports_vision: c.supports_vision,
                supports_tool_use: c.supports_tool_use,
                deprecated: c.deprecated,
                source: "catalog".into(),
            });
        }
    }

    // Layer 2: live `/v1/models` augment. We do not attempt to parse
    // provider-specific structured capability fields here (Gemini /
    // Groq return them but the schemas differ wildly per provider).
    // The catalog is the source of truth for capability values; the
    // live call adds any model IDs the provider exposes that we don't
    // have in the catalog yet.
    let base_url = match provider.base_url.as_deref() {
        Some(b) => b.to_string(),
        None => {
            notes.push("No base_url configured; skipped live /v1/models call".to_string());
            return Ok((
                StatusCode::OK,
                Json(DiscoverModelsResponse {
                    provider_type,
                    models,
                    notes,
                }),
            ));
        }
    };
    let api_key = provider.api_key.clone().unwrap_or_default();

    match fetch_v1_models(&provider_type, &base_url, &api_key).await {
        Ok(live_ids) => {
            for id in live_ids {
                if !models.iter().any(|m| m.id == id) {
                    let registry = ai_providers::registry_lookup(&provider_type, &id);
                    models.push(DiscoveredModel {
                        id: id.clone(),
                        display_name: registry.as_ref().and_then(|c| c.display_name.clone()),
                        context_length: registry.as_ref().and_then(|c| c.context_length),
                        max_output_tokens: registry.as_ref().and_then(|c| c.max_output_tokens),
                        supports_chat: registry.as_ref().map(|c| c.supports_chat).unwrap_or(true),
                        supports_embeddings: registry
                            .as_ref()
                            .map(|c| c.supports_embeddings)
                            .unwrap_or(false),
                        supports_vision: registry
                            .as_ref()
                            .map(|c| c.supports_vision)
                            .unwrap_or(false),
                        supports_tool_use: registry.as_ref().and_then(|c| c.supports_tool_use),
                        deprecated: registry.as_ref().map(|c| c.deprecated).unwrap_or(false),
                        source: if registry.is_some() {
                            "catalog".into()
                        } else {
                            "discovery".into()
                        },
                    });
                }
            }
        }
        Err(e) => {
            notes.push(format!(
                "live /v1/models call failed; falling back to catalog only: {e}"
            ));
        }
    }

    Ok((
        StatusCode::OK,
        Json(DiscoverModelsResponse {
            provider_type,
            models,
            notes,
        }),
    ))
}

/// Best-effort fetch of the provider's `/v1/models`-shaped response.
/// We extract just the model IDs — per-provider capability shape is
/// well-divergent and the catalog covers the structured side.
async fn fetch_v1_models(
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    // SSRF hardening: this request carries the provider's api_key. Threats:
    //   * a 302 to an internal host (e.g. cloud metadata) to harvest the bearer
    //     — blocked by `redirect(Policy::none())` (never follow ANY redirect),
    //   * DNS rebinding (hostname resolves public at pre-flight, rebinds to
    //     169.254.169.254 / loopback / RFC1918 before connect) — blocked by the
    //     connect-time GuardingResolver baked into `validated_client_builder`,
    //   * an ambient proxy tunnelling/seeing the secret — blocked by `no_proxy()`.
    let policy = crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS;
    crate::utils::url_validator::validate_outbound_url(&url, &policy)
        .map_err(|e| format!("blocked url: {e}"))?;
    let client = crate::utils::url_validator::validated_client_builder(policy)
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|e| format!("reqwest build: {e}"))?;
    let req = match provider_type {
        "anthropic" => client.get(&url).header("x-api-key", api_key),
        _ => {
            // OpenAI-compatible default: bearer auth.
            client.get(&url).bearer_auth(api_key)
        }
    };
    let resp = req.send().await.map_err(|e| format!("send: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;

    // OpenAI / Groq / DeepSeek / Mistral: { data: [{id, ...}] }
    // Anthropic:                          { data: [{id, ...}] }
    // Gemini:                             { models: [{name, ...}] }
    let mut ids = Vec::new();
    if let Some(arr) = body.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
        }
    } else if let Some(arr) = body.get("models").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                // Gemini reports "models/gemini-2.0-flash"; trim the prefix.
                let id = name.strip_prefix("models/").unwrap_or(name);
                ids.push(id.to_string());
            }
        }
    }
    Ok(ids)
}

pub fn discover_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmProvidersRead,)>(op)
        .id("LlmProvider.discoverModels")
        .tag("LLM Providers")
        .summary("Discover models for a provider: catalog + live /v1/models + override.")
        .description(concat!(
            "Layered discovery: curated catalog (data/known_models.json), ",
            "augmented with the provider's live /v1/models response, with ",
            "notes about anything that fell back. Local providers are not ",
            "supported here — query /api/llm-models?provider_id= instead."
        ))
        .response::<200, Json<DiscoverModelsResponse>>()
        .response_with::<404, (), _>(|r| r.description("Provider not found"))
}
