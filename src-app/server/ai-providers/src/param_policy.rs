//! Provider-agnostic, declarative model-parameter contract (the REQUEST side of
//! the provider adapter).
//!
//! One unified generation-param set (`ChatRequest`) → each provider family's
//! actual wire field names + allowed/forbidden param set, resolved **by
//! construction** so ziee never sends a param a model rejects. There is NO
//! error-driven self-heal: correctness comes from resolving the contract up
//! front from layered sources (highest priority wins), per capability signal:
//!
//! 1. **DB model-row override** — `ChatRequest.model_caps` (`ModelParamContract`),
//!    user-editable, threaded in by the server.
//! 2. **Curated catalog** — `model_registry::lookup` on `known_models.json`.
//! 3. **Provider model-family policy** — O(families) pattern predicates below;
//!    new models in a known family are auto-covered with no edit.
//! 4. **Conservative default** — the family's standard param set; never inject a
//!    value the user did not set.
//!
//! Request thinking/reasoning semantics are then reconciled on top (only ever
//! *removing*/renaming, never injecting).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::models::{ChatRequest, ThinkingMode};

/// A unified generation parameter, independent of provider wire names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnifiedParam {
    Temperature,
    TopP,
    TopK,
    MaxTokens,
    FrequencyPenalty,
    PresencePenalty,
    Seed,
    Stop,
}

/// Which wire field carries the max-output-token cap for a given provider/model.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    /// Anthropic + classic OpenAI Chat Completions.
    MaxTokens,
    /// OpenAI reasoning / newer models.
    MaxCompletionTokens,
    /// Gemini `generationConfig.maxOutputTokens`.
    MaxOutputTokens,
}

impl MaxTokensField {
    /// The wire field name.
    pub fn key(self) -> &'static str {
        match self {
            MaxTokensField::MaxTokens => "max_tokens",
            MaxTokensField::MaxCompletionTokens => "max_completion_tokens",
            MaxTokensField::MaxOutputTokens => "maxOutputTokens",
        }
    }
}

/// The provider *builder* family (coarser than `provider_type`: one
/// `OpenAiCompat` backs openai/groq/deepseek/mistral/openrouter/…).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProviderFamily {
    Anthropic,
    OpenAiCompat,
    Gemini,
}

impl ProviderFamily {
    /// The `known_models.json` provider key used for the catalog layer. The
    /// OpenAI-compatible family looks up under `"openai"`; non-OpenAI-proper ids
    /// (groq/deepseek/…) simply miss and fall through to the family policy.
    fn catalog_provider(self) -> &'static str {
        match self {
            ProviderFamily::Anthropic => "anthropic",
            ProviderFamily::OpenAiCompat => "openai",
            ProviderFamily::Gemini => "gemini",
        }
    }
}

/// Per-model capability signals supplied by the caller (the server's DB model
/// row) as the top-priority override. Every field is `Option` — `None` means
/// "this source is silent; fall through to catalog → family → default."
///
/// This is the seam that lets a model's contract live on the editable DB row
/// instead of a compiled per-model file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelParamContract {
    /// `Some(false)` ⇒ the model rejects temperature/top_p/top_k.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_sampling_params: Option<bool>,
    /// `Some(true)` ⇒ the model supports thinking/reasoning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_thinking: Option<bool>,
    /// `"adaptive"` | `"budget"` — how thinking is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_style: Option<String>,
    /// Override the max-tokens wire field (rare; usually family-derived).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<MaxTokensField>,
}

/// The resolved decision set a `build_request_body` consumes. Carries *decisions
/// only* — each provider maps them onto its own body shape.
#[derive(Clone, Debug)]
pub struct ResolvedParams {
    eligible: HashSet<UnifiedParam>,
    /// Which wire field the max-token cap goes under.
    pub max_tokens_field: MaxTokensField,
    /// Emit `reasoning_effort` (OpenAI reasoning models).
    pub use_reasoning_effort: bool,
    /// Force a non-streaming request (OpenAI gpt-5 org-verification quirk).
    pub disable_stream: bool,
}

impl ResolvedParams {
    /// Whether `p` may be emitted on the wire for this model/request.
    pub fn allows(&self, p: UnifiedParam) -> bool {
        self.eligible.contains(&p)
    }
}

// ---------------------------------------------------------------------------
// Model-family pattern policy (O(families), stable across model releases).
// ---------------------------------------------------------------------------

/// Strip a leading gateway/vendor prefix (`openai/o3` → `o3`) + lowercase.
fn bare_id(model_id: &str) -> String {
    let lower = model_id.to_ascii_lowercase();
    lower.rsplit('/').next().unwrap_or(&lower).to_string()
}

/// OpenAI o-series reasoning models: `o1`, `o3`, `o4-mini`, … (an `o` followed
/// by a digit, optionally `-suffix`). Excludes ordinary ids like `omni`.
fn is_o_series(bare: &str) -> bool {
    let rest = match bare.strip_prefix('o') {
        Some(r) => r,
        None => return false,
    };
    let first = rest.chars().next();
    matches!(first, Some(c) if c.is_ascii_digit())
}

/// True when the OpenAI-compatible model is a reasoning model by family
/// convention (o-series or gpt-5*), independent of the request's thinking flag.
/// `gpt-5-chat*` is the documented non-reasoning exception.
pub fn openai_reasoning_family(model_id: &str) -> bool {
    let m = bare_id(model_id);
    if m.starts_with("gpt-5-chat") {
        return false;
    }
    is_o_series(&m) || m.starts_with("gpt-5")
}

/// OpenAI models that reject `stream: true` (org-verification), requiring a
/// single non-streamed body fanned out into chunks. Seeded from the former
/// `MODELS_REQUIRING_NON_STREAMING` const.
pub fn openai_requires_non_streaming(model_id: &str) -> bool {
    let m = bare_id(model_id);
    m == "gpt-5" || m == "gpt-5-mini"
}

/// Anthropic tiers that reject sampling params (temperature/top_p/top_k) by
/// family convention: Opus-class (any generation) and Claude-5+.
pub fn anthropic_sampling_restricted(model_id: &str) -> bool {
    let m = bare_id(model_id);
    if !m.starts_with("claude-") {
        return false;
    }
    if m.contains("opus") {
        return true;
    }
    // claude-<tier>-<major>… where major >= 5 (e.g. claude-sonnet-5).
    claude_major_version(&m).is_some_and(|v| v >= 5)
}

/// Extract the leading major version integer that follows the model tier in a
/// `claude-<tier>-<major>[-…]` id (e.g. `claude-sonnet-5-2026… → 5`).
fn claude_major_version(bare: &str) -> Option<u32> {
    // Segments after "claude": tier(s) then a numeric major.
    for seg in bare.split('-').skip(1) {
        if let Ok(n) = seg.parse::<u32>() {
            return Some(n);
        }
    }
    None
}

/// Family-pattern thinking support: returns the inferred `thinking_style`
/// (`"adaptive"`) for families known to support reasoning, else `None`.
fn family_thinking_style(family: ProviderFamily, model_id: &str) -> Option<&'static str> {
    let m = bare_id(model_id);
    match family {
        ProviderFamily::Anthropic => {
            // Opus (all), Sonnet 4.x+, and any Claude 5+ tier support adaptive
            // thinking. Haiku does NOT — so it is deliberately excluded (a bare
            // `major >= 4` would wrongly enable it for haiku-4-5).
            if m.contains("opus") {
                return Some("adaptive");
            }
            if claude_major_version(&m).is_some_and(|v| v >= 5) {
                return Some("adaptive");
            }
            if m.contains("sonnet") && claude_major_version(&m).is_some_and(|v| v >= 4) {
                return Some("adaptive");
            }
            None
        }
        ProviderFamily::OpenAiCompat => openai_reasoning_family(model_id).then_some("adaptive"),
        ProviderFamily::Gemini => m.starts_with("gemini-2.5").then_some("adaptive"),
    }
}

// ---------------------------------------------------------------------------
// Catalog layer.
// ---------------------------------------------------------------------------

fn catalog_supports_sampling(family: ProviderFamily, model_id: &str) -> Option<bool> {
    crate::model_registry::lookup(family.catalog_provider(), model_id)
        .and_then(|c| c.supports_sampling_params)
}

fn catalog_thinking(family: ProviderFamily, model_id: &str) -> Option<(bool, Option<String>)> {
    crate::model_registry::lookup(family.catalog_provider(), model_id)
        .and_then(|c| c.supports_thinking.map(|s| (s, c.thinking_style)))
}

// ---------------------------------------------------------------------------
// Resolution.
// ---------------------------------------------------------------------------

/// Whether the model rejects sampling params, resolved: row-override → catalog
/// → family pattern → default(allowed).
fn sampling_restricted(family: ProviderFamily, model_id: &str, contract: &ModelParamContract) -> bool {
    if let Some(supported) = contract.supports_sampling_params {
        return !supported;
    }
    if let Some(supported) = catalog_supports_sampling(family, model_id) {
        return !supported;
    }
    match family {
        ProviderFamily::Anthropic => anthropic_sampling_restricted(model_id),
        // OpenAI reasoning models reject sampling; the reasoning branch also
        // drops it, but be explicit so a non-thinking reasoning id is covered.
        ProviderFamily::OpenAiCompat => openai_reasoning_family(model_id),
        ProviderFamily::Gemini => false,
    }
}

/// Resolve the model's thinking style, row-override → catalog → family pattern.
/// `None` ⇒ thinking not supported / unknown ⇒ do not enable it.
///
/// Used by the server's `thinking_config_for` so the thinking decision is
/// dynamic (a user can enable it on the DB row for an uncatalogued model).
pub fn resolved_thinking_style(
    family: ProviderFamily,
    model_id: &str,
    contract: &ModelParamContract,
) -> Option<String> {
    // Row override: an explicit supports_thinking wins.
    if let Some(supported) = contract.supports_thinking {
        if !supported {
            return None;
        }
        return Some(
            contract
                .thinking_style
                .clone()
                .or_else(|| family_thinking_style(family, model_id).map(str::to_string))
                .unwrap_or_else(|| "adaptive".to_string()),
        );
    }
    // Catalog.
    if let Some((supported, style)) = catalog_thinking(family, model_id) {
        if !supported {
            return None;
        }
        return Some(style.unwrap_or_else(|| "adaptive".to_string()));
    }
    // Family pattern.
    family_thinking_style(family, model_id).map(str::to_string)
}

/// The family's base allowed param set + default `max_tokens` field.
fn base_spec(family: ProviderFamily) -> ResolvedParams {
    use UnifiedParam::*;
    let (eligible, field): (&[UnifiedParam], MaxTokensField) = match family {
        // Anthropic: no penalties, no seed; no top_k restriction here.
        ProviderFamily::Anthropic => (
            &[Temperature, TopP, TopK, MaxTokens, Stop],
            MaxTokensField::MaxTokens,
        ),
        // OpenAI Chat Completions: no top_k.
        ProviderFamily::OpenAiCompat => (
            &[
                Temperature,
                TopP,
                MaxTokens,
                FrequencyPenalty,
                PresencePenalty,
                Seed,
                Stop,
            ],
            MaxTokensField::MaxTokens,
        ),
        // Gemini: everything, via generationConfig.
        ProviderFamily::Gemini => (
            &[
                Temperature,
                TopP,
                TopK,
                MaxTokens,
                FrequencyPenalty,
                PresencePenalty,
                Seed,
                Stop,
            ],
            MaxTokensField::MaxOutputTokens,
        ),
    };
    ResolvedParams {
        eligible: eligible.iter().copied().collect(),
        max_tokens_field: field,
        use_reasoning_effort: false,
        disable_stream: false,
    }
}

fn drop_sampling(rp: &mut ResolvedParams) {
    rp.eligible.remove(&UnifiedParam::Temperature);
    rp.eligible.remove(&UnifiedParam::TopP);
    rp.eligible.remove(&UnifiedParam::TopK);
}

/// Resolve the full parameter decision set for a request. See the module docs
/// for the layering. Later layers only ever remove/rename — never inject.
pub fn resolve(
    family: ProviderFamily,
    model_id: &str,
    req: &ChatRequest,
    contract: &ModelParamContract,
) -> ResolvedParams {
    let mut rp = base_spec(family);

    // Capability: sampling support (row → catalog → family → default).
    if sampling_restricted(family, model_id, contract) {
        drop_sampling(&mut rp);
    }
    // Contract may override the max-tokens field explicitly.
    if let Some(field) = contract.max_tokens_field {
        rp.max_tokens_field = field;
    }

    let thinking_active = matches!(
        req.thinking.as_ref().map(|t| t.mode),
        Some(ThinkingMode::Adaptive | ThinkingMode::Enabled)
    );

    match family {
        ProviderFamily::OpenAiCompat => {
            // Reasoning model: by family convention OR because thinking is on.
            let reasoning = openai_reasoning_family(model_id) || thinking_active;
            if reasoning {
                rp.max_tokens_field = MaxTokensField::MaxCompletionTokens;
                rp.use_reasoning_effort = thinking_active;
                rp.eligible.remove(&UnifiedParam::Temperature);
                rp.eligible.remove(&UnifiedParam::TopP);
                rp.eligible.remove(&UnifiedParam::FrequencyPenalty);
                rp.eligible.remove(&UnifiedParam::PresencePenalty);
            }
            // gpt-5 org-verification: non-streaming + no sampling/seed/stop.
            if openai_requires_non_streaming(model_id) {
                rp.disable_stream = true;
                rp.max_tokens_field = MaxTokensField::MaxCompletionTokens;
                drop_sampling(&mut rp);
                rp.eligible.remove(&UnifiedParam::FrequencyPenalty);
                rp.eligible.remove(&UnifiedParam::PresencePenalty);
                rp.eligible.remove(&UnifiedParam::Seed);
                rp.eligible.remove(&UnifiedParam::Stop);
            }
        }
        ProviderFamily::Anthropic => {
            // Adaptive/extended thinking requires temperature==1 (or omitted);
            // omit sampling entirely so Anthropic applies its default.
            if thinking_active {
                drop_sampling(&mut rp);
            }
        }
        // Gemini accepts sampling alongside thinkingConfig; nothing to drop.
        ProviderFamily::Gemini => {}
    }

    rp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ThinkingConfig, ThinkingMode};

    fn req_with_thinking(model: &str, mode: Option<ThinkingMode>) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(100),
            thinking: mode.map(|m| ThinkingConfig {
                mode: m,
                budget_tokens: None,
                effort: None,
                include_thinking: true,
            }),
            ..Default::default()
        }
    }

    // TEST-2: field-name map per family.
    #[test]
    fn max_tokens_field_per_family() {
        let none = ModelParamContract::default();
        // OpenAI non-reasoning → max_tokens.
        let rp = resolve(ProviderFamily::OpenAiCompat, "gpt-4o", &req_with_thinking("gpt-4o", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxTokens);
        assert!(!rp.disable_stream);
        // OpenAI reasoning → max_completion_tokens.
        let rp = resolve(ProviderFamily::OpenAiCompat, "o3-mini", &req_with_thinking("o3-mini", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxCompletionTokens);
        // gpt-5 → max_completion_tokens + disable_stream.
        let rp = resolve(ProviderFamily::OpenAiCompat, "gpt-5", &req_with_thinking("gpt-5", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxCompletionTokens);
        assert!(rp.disable_stream);
        // Anthropic → max_tokens.
        let rp = resolve(ProviderFamily::Anthropic, "claude-sonnet-4-6", &req_with_thinking("claude-sonnet-4-6", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxTokens);
        // Gemini → maxOutputTokens.
        let rp = resolve(ProviderFamily::Gemini, "gemini-2.5-flash", &req_with_thinking("gemini-2.5-flash", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxOutputTokens);
        assert_eq!(MaxTokensField::MaxCompletionTokens.key(), "max_completion_tokens");
        assert_eq!(MaxTokensField::MaxOutputTokens.key(), "maxOutputTokens");
    }

    // TEST-3: reconciliation rules.
    #[test]
    fn reconciliation_thinking_and_reasoning() {
        let none = ModelParamContract::default();
        // Anthropic thinking-active ⇒ sampling ineligible.
        let rp = resolve(
            ProviderFamily::Anthropic,
            "claude-sonnet-4-6",
            &req_with_thinking("claude-sonnet-4-6", Some(ThinkingMode::Adaptive)),
            &none,
        );
        assert!(!rp.allows(UnifiedParam::Temperature));
        assert!(!rp.allows(UnifiedParam::TopP));
        assert!(!rp.allows(UnifiedParam::TopK));
        // Without thinking, sonnet-4-6 allows sampling (catalog says supported).
        let rp = resolve(
            ProviderFamily::Anthropic,
            "claude-sonnet-4-6",
            &req_with_thinking("claude-sonnet-4-6", None),
            &none,
        );
        assert!(rp.allows(UnifiedParam::Temperature));
        // OpenAI reasoning ⇒ sampling+penalties ineligible + reasoning effort.
        let rp = resolve(
            ProviderFamily::OpenAiCompat,
            "o3",
            &req_with_thinking("o3", Some(ThinkingMode::Adaptive)),
            &none,
        );
        assert!(!rp.allows(UnifiedParam::Temperature));
        assert!(!rp.allows(UnifiedParam::FrequencyPenalty));
        assert!(rp.use_reasoning_effort);
    }

    // TEST-4: family patterns generalize with no catalog entry.
    #[test]
    fn family_patterns_generalize() {
        let none = ModelParamContract::default();
        // A brand-new o-series id, absent from the catalog, still reasoning.
        assert!(openai_reasoning_family("o5-mini"));
        let rp = resolve(ProviderFamily::OpenAiCompat, "o5-mini", &req_with_thinking("o5-mini", None), &none);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxCompletionTokens);
        assert!(!rp.allows(UnifiedParam::Temperature));
        // A brand-new opus id, absent from the catalog, still sampling-restricted.
        assert!(anthropic_sampling_restricted("claude-opus-4-9"));
        let rp = resolve(ProviderFamily::Anthropic, "claude-opus-4-9", &req_with_thinking("claude-opus-4-9", None), &none);
        assert!(!rp.allows(UnifiedParam::Temperature));
        // A future claude-6 tier.
        assert!(anthropic_sampling_restricted("claude-sonnet-6"));
        // A plain chat id passes sampling through.
        let rp = resolve(ProviderFamily::OpenAiCompat, "gpt-4o-mini", &req_with_thinking("gpt-4o-mini", None), &none);
        assert!(rp.allows(UnifiedParam::Temperature));
        // gpt-5-chat is the non-reasoning exception.
        assert!(!openai_reasoning_family("gpt-5-chat-latest"));
        // `omni`/`ollama`-style ids are NOT o-series.
        assert!(!is_o_series("omni"));
    }

    // TEST-1 + TEST-5: precedence + graceful degrade.
    #[test]
    fn precedence_and_graceful_degrade() {
        // Row override beats family pattern: force sampling ON for an opus id.
        let allow = ModelParamContract {
            supports_sampling_params: Some(true),
            ..Default::default()
        };
        let rp = resolve(ProviderFamily::Anthropic, "claude-opus-4-9", &req_with_thinking("claude-opus-4-9", None), &allow);
        assert!(rp.allows(UnifiedParam::Temperature), "row override should re-enable sampling");
        // Row override beats catalog: force sampling OFF for a normally-allowed model.
        let deny = ModelParamContract {
            supports_sampling_params: Some(false),
            ..Default::default()
        };
        let rp = resolve(ProviderFamily::OpenAiCompat, "gpt-4o", &req_with_thinking("gpt-4o", None), &deny);
        assert!(!rp.allows(UnifiedParam::Temperature), "row override should disable sampling");
        // Graceful degrade: unknown model, no pattern, no override, no thinking ⇒ params pass through.
        let none = ModelParamContract::default();
        let rp = resolve(ProviderFamily::OpenAiCompat, "some-unknown-model-xyz", &req_with_thinking("some-unknown-model-xyz", None), &none);
        assert!(rp.allows(UnifiedParam::Temperature));
        assert!(rp.allows(UnifiedParam::TopP));
        // OpenAI-compat never eligible for top_k.
        assert!(!rp.allows(UnifiedParam::TopK));
        // Contract max_tokens_field override honored.
        let mtf = ModelParamContract {
            max_tokens_field: Some(MaxTokensField::MaxCompletionTokens),
            ..Default::default()
        };
        let rp = resolve(ProviderFamily::OpenAiCompat, "custom-x", &req_with_thinking("custom-x", None), &mtf);
        assert_eq!(rp.max_tokens_field, MaxTokensField::MaxCompletionTokens);
    }

    #[test]
    fn thinking_style_resolution_layers() {
        // Row override wins.
        let row = ModelParamContract {
            supports_thinking: Some(true),
            thinking_style: Some("budget".to_string()),
            ..Default::default()
        };
        assert_eq!(
            resolved_thinking_style(ProviderFamily::OpenAiCompat, "mystery", &row).as_deref(),
            Some("budget")
        );
        // Row override disabling wins over a thinking-capable family.
        let off = ModelParamContract {
            supports_thinking: Some(false),
            ..Default::default()
        };
        assert_eq!(resolved_thinking_style(ProviderFamily::Anthropic, "claude-opus-4-9", &off), None);
        // Family pattern fallback (no catalog, no row).
        let none = ModelParamContract::default();
        assert_eq!(
            resolved_thinking_style(ProviderFamily::Anthropic, "claude-opus-4-9", &none).as_deref(),
            Some("adaptive")
        );
        assert_eq!(resolved_thinking_style(ProviderFamily::OpenAiCompat, "gpt-4o", &none), None);
    }
}
