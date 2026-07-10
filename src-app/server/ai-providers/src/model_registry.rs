//! Curated catalog of remote provider models.
//!
//! Loaded at first use from `data/known_models.json` (a copy
//! bundled into the binary at build time via `include_str!`).
//! Subsequent lookups hit an `HashMap<(provider, model_id),
//! ModelCapabilities>` so the read path is O(1).
//!
//! P1.j of feat/local-llm-runtime.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Shape that mirrors the entries in `known_models.json`. Open-ended
/// — we accept and forward unknown fields untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub id: Option<String>,
    pub display_name: Option<String>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub supports_chat: bool,
    #[serde(default)]
    pub supports_embeddings: bool,
    #[serde(default)]
    pub supports_vision: bool,
    pub supports_tool_use: Option<bool>,
    /// Whether the model supports thinking/reasoning. `None` = unknown.
    pub supports_thinking: Option<bool>,
    /// How thinking is requested: `"adaptive"` (modern Anthropic / Gemini 2.5 /
    /// OpenAI reasoning models) or `"budget"` (legacy fixed token budget).
    pub thinking_style: Option<String>,
    /// Whether the model accepts sampling params (`temperature`/`top_p`/`top_k`).
    /// `Some(false)` for Anthropic Opus 4.7/4.8 (they 400 on these). `None` =
    /// unknown → treated as allowed.
    pub supports_sampling_params: Option<bool>,
    #[serde(default)]
    pub deprecated: bool,
}

const RAW: &str = include_str!("../data/known_models.json");

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    #[serde(flatten)]
    providers: HashMap<String, serde_json::Value>,
}

fn registry() -> &'static HashMap<(String, String), ModelCapabilities> {
    static REG: OnceLock<HashMap<(String, String), ModelCapabilities>> = OnceLock::new();
    REG.get_or_init(|| {
        let cat: Catalog = match serde_json::from_str(RAW) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("model_registry: failed to parse known_models.json: {e}");
                return HashMap::new();
            }
        };
        let mut out = HashMap::new();
        for (provider, list_value) in cat.providers.into_iter() {
            if provider.starts_with('_') {
                continue; // skip _schema_notes etc.
            }
            let list: Vec<ModelCapabilities> = match serde_json::from_value(list_value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "model_registry: failed to parse list for {provider}: {e}"
                    );
                    continue;
                }
            };
            for entry in list {
                if let Some(id) = entry.id.clone() {
                    out.insert((provider.clone(), id), entry);
                }
            }
        }
        out
    })
}

/// Look up known capabilities for a (provider, model_id) pair.
///
/// Tries an exact match first, then falls back to the longest catalog id that is
/// a dash-prefixed base of `model_id` — so dated/aliased SKUs
/// (`claude-opus-4-7-20250514`, `claude-opus-4-7-fast`) still resolve to the
/// `claude-opus-4-7` entry. Without this, the capability gates (e.g. Anthropic's
/// `supports_sampling_params`) would silently miss dated ids.
pub fn lookup(provider_type: &str, model_id: &str) -> Option<ModelCapabilities> {
    let reg = registry();
    if let Some(c) = reg.get(&(provider_type.to_string(), model_id.to_string())) {
        return Some(c.clone());
    }
    reg.iter()
        .filter(|((p, id), _)| p == provider_type && model_id.starts_with(&format!("{id}-")))
        .max_by_key(|((_, id), _)| id.len())
        .map(|(_, c)| c.clone())
}

/// All known model IDs for a provider type. The drawer dropdown
/// falls back to this when the live `/v1/models` call fails.
pub fn known_ids_for(provider_type: &str) -> Vec<String> {
    registry()
        .iter()
        .filter_map(|((p, id), _)| {
            if p == provider_type {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_gpt4o_known() {
        let c = lookup("openai", "gpt-4o").expect("gpt-4o should be in registry");
        assert_eq!(c.context_length, Some(128000));
        assert!(c.supports_chat);
        assert!(c.supports_vision);
    }

    #[test]
    fn anthropic_claude_opus_known() {
        let c = lookup("anthropic", "claude-opus-4-7").unwrap();
        assert_eq!(c.context_length, Some(1000000));
    }

    #[test]
    fn unknown_returns_none() {
        assert!(lookup("openai", "no-such-model").is_none());
        assert!(lookup("unknown-provider", "anything").is_none());
    }

    // TEST-9: the curated catalog carries claude-sonnet-5 as sampling-restricted,
    // and a dated SKU resolves to the base entry (prefix-tolerant lookup).
    #[test]
    fn sonnet_5_sampling_restricted_and_dated_resolves() {
        let c = lookup("anthropic", "claude-sonnet-5").expect("sonnet-5 in catalog");
        assert_eq!(c.supports_sampling_params, Some(false));
        assert_eq!(c.supports_thinking, Some(true));
        assert_eq!(c.thinking_style.as_deref(), Some("adaptive"));
        let dated = lookup("anthropic", "claude-sonnet-5-20260115").expect("dated sonnet-5 resolves");
        assert_eq!(dated.supports_sampling_params, Some(false));
    }

    #[test]
    fn known_ids_for_openai_nonempty() {
        let ids = known_ids_for("openai");
        assert!(ids.contains(&"gpt-4o".to_string()));
    }

    #[test]
    fn embedding_model_flagged() {
        let c = lookup("openai", "text-embedding-3-small").unwrap();
        assert!(c.supports_embeddings);
        assert!(!c.supports_chat);
    }

    #[test]
    fn opus_47_thinking_adaptive_and_sampling_restricted() {
        let c = lookup("anthropic", "claude-opus-4-7").unwrap();
        assert_eq!(c.supports_thinking, Some(true));
        assert_eq!(c.thinking_style.as_deref(), Some("adaptive"));
        // must-fix gate signal: Opus 4.7 rejects sampling params.
        assert_eq!(c.supports_sampling_params, Some(false));
    }

    #[test]
    fn sonnet_46_thinking_adaptive_sampling_allowed() {
        let c = lookup("anthropic", "claude-sonnet-4-6").unwrap();
        assert_eq!(c.supports_thinking, Some(true));
        assert_eq!(c.supports_sampling_params, Some(true));
    }

    #[test]
    fn gemini_25_supports_thinking() {
        let c = lookup("gemini", "gemini-2.5-flash").unwrap();
        assert_eq!(c.supports_thinking, Some(true));
        assert_eq!(c.thinking_style.as_deref(), Some("adaptive"));
    }

    #[test]
    fn non_thinking_model_unset() {
        let c = lookup("openai", "gpt-4o").unwrap();
        assert!(c.supports_thinking.is_none());
    }

    #[test]
    fn dated_and_aliased_ids_resolve_to_base_entry() {
        // Dated SKU → the bare catalog entry (so the sampling gate still fires).
        let c = lookup("anthropic", "claude-opus-4-7-20250514").expect("dated id resolves");
        assert_eq!(c.supports_sampling_params, Some(false));
        let c2 = lookup("anthropic", "claude-opus-4-7-fast").expect("aliased id resolves");
        assert_eq!(c2.supports_thinking, Some(true));
        // A non-dash continuation must NOT false-match.
        assert!(lookup("anthropic", "claude-opus-4-70").is_none());
    }
}
