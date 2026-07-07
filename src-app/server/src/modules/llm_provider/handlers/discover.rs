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

/// A model as reported by a provider's live `/v1/models` response, with any
/// richer capability fields we can parse. Fields are `Option` because most
/// OpenAI-compatible `/models` responses list IDs only — OpenRouter (and, to a
/// lesser degree, Gemini/Groq) carry structured capability/context data.
///
/// `pub(crate)` so the deprecation sweep (`llm_model::prune`) can reuse the same
/// fetch to compute the live model-id set for a provider.
#[derive(Debug, Clone, Default)]
pub(crate) struct LiveModel {
    pub id: String,
    pub display_name: Option<String>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_vision: Option<bool>,
    pub supports_tool_use: Option<bool>,
    pub supports_embeddings: Option<bool>,
}

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

    match fetch_live_models(&provider_type, &base_url, &api_key).await {
        Ok(live) => {
            for lm in live {
                if !models.iter().any(|m| m.id == lm.id) {
                    // Catalog stays the source of truth for capability values;
                    // for IDs the catalog does not know we prefer the live-parsed
                    // fields (OpenRouter etc.) and fall back to registry/defaults.
                    let registry = ai_providers::registry_lookup(&provider_type, &lm.id);
                    let embeddings_hint = lm.supports_embeddings.unwrap_or(false);
                    models.push(DiscoveredModel {
                        id: lm.id.clone(),
                        display_name: lm
                            .display_name
                            .clone()
                            .or_else(|| registry.as_ref().and_then(|c| c.display_name.clone())),
                        context_length: lm
                            .context_length
                            .or_else(|| registry.as_ref().and_then(|c| c.context_length)),
                        max_output_tokens: lm
                            .max_output_tokens
                            .or_else(|| registry.as_ref().and_then(|c| c.max_output_tokens)),
                        // A pure-embeddings model isn't a chat model; otherwise
                        // default chat=true for a discovered generative model.
                        supports_chat: registry
                            .as_ref()
                            .map(|c| c.supports_chat)
                            .unwrap_or(!embeddings_hint),
                        supports_embeddings: lm
                            .supports_embeddings
                            .or_else(|| registry.as_ref().map(|c| c.supports_embeddings))
                            .unwrap_or(false),
                        supports_vision: lm
                            .supports_vision
                            .or_else(|| registry.as_ref().map(|c| c.supports_vision))
                            .unwrap_or(false),
                        supports_tool_use: lm
                            .supports_tool_use
                            .or_else(|| registry.as_ref().and_then(|c| c.supports_tool_use)),
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

/// SSRF policy for the live `/v1/models` fetch. `PUBLIC_HTTP_OR_HTTPS` in
/// release builds. A debug-only env seam (`LLM_DISCOVER_ALLOW_LOOPBACK`) relaxes
/// it to `DEV_LOCAL` so integration tests can point a provider at a 127.0.0.1
/// mock `/models` — compiled out of release builds via `cfg!(debug_assertions)`,
/// mirroring `web_search`'s `WEB_SEARCH_FETCH_ALLOW_LOOPBACK`. RFC1918 /
/// link-local / IMDS stay blocked even under `DEV_LOCAL`.
fn discovery_url_policy() -> crate::utils::url_validator::OutboundUrlPolicy {
    #[cfg(debug_assertions)]
    {
        if std::env::var("LLM_DISCOVER_ALLOW_LOOPBACK").is_ok() {
            return crate::utils::url_validator::OutboundUrlPolicy::DEV_LOCAL;
        }
    }
    crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
}

/// Best-effort fetch of the provider's `/v1/models`-shaped response, parsed into
/// [`LiveModel`]s. Most OpenAI-compatible providers list IDs only; OpenRouter
/// (and Gemini/Groq) carry structured context/capability fields we surface when
/// present. `pub(crate)` so the deprecation sweep reuses the exact same fetch.
pub(crate) async fn fetch_live_models(
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<LiveModel>, String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    // SSRF hardening: this request carries the provider's api_key. Threats:
    //   * a 302 to an internal host (e.g. cloud metadata) to harvest the bearer
    //     — blocked by `redirect(Policy::none())` (never follow ANY redirect),
    //   * DNS rebinding (hostname resolves public at pre-flight, rebinds to
    //     169.254.169.254 / loopback / RFC1918 before connect) — blocked by the
    //     connect-time GuardingResolver baked into `validated_client_builder`,
    //   * an ambient proxy tunnelling/seeing the secret — blocked by `no_proxy()`.
    let policy = discovery_url_policy();
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
    Ok(parse_live_models(provider_type, &body))
}

/// Pure parser for a `/models` response body → [`LiveModel`]s. Network-free so
/// it is directly unit-testable. Handles the two response shapes plus the richer
/// OpenRouter/OpenAI-compatible per-model fields (all optional):
///
/// * OpenAI / Groq / DeepSeek / Mistral / OpenRouter / Anthropic: `{ data: [{id, ...}] }`
/// * Gemini: `{ models: [{name, ...}] }` (the `models/` prefix is trimmed)
///
/// Rich fields when present: `name`→display_name, top-level `context_length`,
/// `top_provider.max_completion_tokens`→max_output_tokens,
/// `architecture.input_modalities` containing `"image"`→vision,
/// `supported_parameters` containing `"tools"`→tool_use. `pricing` is
/// intentionally ignored (DEC-4: no pricing).
pub(crate) fn parse_live_models(provider_type: &str, body: &serde_json::Value) -> Vec<LiveModel> {
    let mut out = Vec::new();
    if let Some(arr) = body.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            out.push(parse_one_live_model(id.to_string(), item));
        }
    } else if let Some(arr) = body.get("models").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                // Gemini reports "models/gemini-2.0-flash"; trim the prefix.
                let id = name.strip_prefix("models/").unwrap_or(name).to_string();
                out.push(parse_one_live_model(id, item));
            }
        }
    }
    let _ = provider_type; // reserved for future provider-specific shapes
    out
}

fn parse_one_live_model(id: String, item: &serde_json::Value) -> LiveModel {
    // `name` is only useful as a display name when it differs from the id
    // (OpenRouter sets a human label; OpenAI omits it).
    let display_name = item
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|n| !n.is_empty() && *n != id)
        .map(|s| s.to_string());

    let context_length = item
        .get("context_length")
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok());

    let max_output_tokens = item
        .get("top_provider")
        .and_then(|tp| tp.get("max_completion_tokens"))
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok());

    let supports_vision = item
        .get("architecture")
        .and_then(|a| a.get("input_modalities"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .any(|m| m.eq_ignore_ascii_case("image"))
        });
    // `/models` responses carry no reliable embeddings signal (embeddings is an
    // OUTPUT modality, and no common provider tags it on `input_modalities`), so
    // we do not infer it from the live response — the curated catalog is the
    // source of truth for embeddings capability.
    let supports_embeddings = None;

    let supports_tool_use = item
        .get("supported_parameters")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .any(|p| p == "tools" || p == "tool_choice")
        });

    LiveModel {
        id,
        display_name,
        context_length,
        max_output_tokens,
        supports_vision,
        supports_tool_use,
        supports_embeddings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_openrouter_rich_fields() {
        // Shape captured from the live OpenRouter /api/v1/models response.
        let body = json!({
            "data": [{
                "id": "anthropic/claude-sonnet-5",
                "name": "Anthropic: Claude Sonnet 5",
                "context_length": 200000,
                "architecture": { "input_modalities": ["text", "image"] },
                "supported_parameters": ["tools", "temperature", "max_tokens"],
                "top_provider": { "max_completion_tokens": 64000 },
                "pricing": { "prompt": "0.000003", "completion": "0.000015" }
            }]
        });
        let out = parse_live_models("openrouter", &body);
        assert_eq!(out.len(), 1);
        let m = &out[0];
        assert_eq!(m.id, "anthropic/claude-sonnet-5");
        assert_eq!(m.display_name.as_deref(), Some("Anthropic: Claude Sonnet 5"));
        assert_eq!(m.context_length, Some(200000));
        assert_eq!(m.max_output_tokens, Some(64000));
        assert_eq!(m.supports_vision, Some(true));
        assert_eq!(m.supports_tool_use, Some(true));
        // pricing must never leak into a LiveModel field.
    }

    #[test]
    fn tool_and_vision_negatives() {
        let body = json!({
            "data": [{
                "id": "some/text-only",
                "context_length": 8192,
                "architecture": { "input_modalities": ["text"] },
                "supported_parameters": ["temperature", "max_tokens"]
            }]
        });
        let m = &parse_live_models("openrouter", &body)[0];
        assert_eq!(m.supports_vision, Some(false));
        assert_eq!(m.supports_tool_use, Some(false));
        assert_eq!(m.context_length, Some(8192));
    }

    #[test]
    fn plain_openai_ids_only() {
        // OpenAI's /v1/models lists ids with no capability fields.
        let body = json!({
            "data": [
                { "id": "gpt-4o", "object": "model", "owned_by": "openai" },
                { "id": "text-embedding-3-small", "object": "model" }
            ]
        });
        let out = parse_live_models("openai", &body);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "gpt-4o");
        assert!(out[0].display_name.is_none());
        assert!(out[0].context_length.is_none());
        assert!(out[0].supports_vision.is_none());
        assert!(out[0].supports_tool_use.is_none());
    }

    #[test]
    fn gemini_models_shape_strips_prefix() {
        let body = json!({
            "models": [
                { "name": "models/gemini-2.5-flash" },
                { "name": "models/gemini-2.0-flash" }
            ]
        });
        let out = parse_live_models("gemini", &body);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "gemini-2.5-flash");
        assert_eq!(out[1].id, "gemini-2.0-flash");
    }
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
