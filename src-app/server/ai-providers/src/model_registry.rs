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
    #[serde(default)]
    pub deprecated: bool,
    /// Marker so the UI can show "registry hit" vs "discovery hit".
    pub unknown_to_registry: Option<bool>,
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
pub fn lookup(provider_type: &str, model_id: &str) -> Option<ModelCapabilities> {
    registry()
        .get(&(provider_type.to_string(), model_id.to_string()))
        .cloned()
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
        assert_eq!(c.context_length, Some(200000));
    }

    #[test]
    fn unknown_returns_none() {
        assert!(lookup("openai", "no-such-model").is_none());
        assert!(lookup("unknown-provider", "anything").is_none());
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
}
