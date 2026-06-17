//! Search-provider registry + ordered fallback-chain dispatch.
//!
//! The *set* of supported engines lives HERE (in `catalog()`), not in the DB.
//! The `web_search_providers` table only stores `{api_key, config}` keyed by a
//! registry key. Adding an engine = implement [`SearchProvider`], add a
//! [`ProviderDescriptor`] to `CATALOG`, and a `build()` arm — no migration, no
//! frontend change (the admin UI renders from the descriptor catalog).

pub mod brave;
pub mod searxng;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::common::AppError;
use crate::core::Repos;

/// Hard cap on a search provider's JSON response body. Search results are small;
/// this bounds memory against a misbehaving/compromised endpoint (parity with
/// the page-fetch byte cap).
pub(super) const MAX_SEARCH_BODY_BYTES: u64 = 4 * 1024 * 1024;

/// Read + deserialize an upstream JSON response with a hard byte cap (reqwest
/// has no default body limit). Shared by the search providers so neither can
/// drift into an unbounded `resp.json()`.
pub(super) async fn read_json_capped<T: DeserializeOwned>(
    resp: reqwest::Response,
    max_bytes: u64,
) -> Result<T, AppError> {
    if let Some(len) = resp.content_length()
        && len > max_bytes
    {
        return Err(AppError::internal_error(format!(
            "search response too large: {len} bytes (cap {max_bytes})"
        )));
    }
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| AppError::internal_error(format!("search response read failed: {e}")))?;
        if buf.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(AppError::internal_error(format!(
                "search response exceeds size cap ({max_bytes} bytes)"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&buf)
        .map_err(|e| AppError::internal_error(format!("search response parse failed: {e}")))
}

use super::models::WebSearchSettings;
use super::repository::WebSearchProviderRow;

/// One ranked search result. Returned to the model via `structuredContent`.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Result of a chain search: the hits + which engine actually served them.
#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub provider: String,
    pub results: Vec<SearchHit>,
}

/// A pluggable search engine. One instance is built per request from the
/// provider's stored config + key.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, count: usize) -> Result<Vec<SearchHit>, AppError>;
}

/// A non-secret config field a provider needs — drives the generic admin UI.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ConfigField {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub placeholder: &'static str,
}

/// Static, code-owned description of one engine.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ProviderDescriptor {
    pub key: &'static str,
    pub display_name: &'static str,
    pub needs_api_key: bool,
    pub config_fields: Vec<ConfigField>,
}

/// The catalog of engines this build supports. Append here to add an engine.
pub fn catalog() -> Vec<ProviderDescriptor> {
    vec![
        ProviderDescriptor {
            key: "searxng",
            display_name: "SearXNG (self-hosted)",
            needs_api_key: false,
            config_fields: vec![ConfigField {
                key: "base_url",
                label: "Base URL",
                required: true,
                placeholder: "https://searxng.example.com",
            }],
        },
        ProviderDescriptor {
            key: "brave",
            display_name: "Brave Search",
            needs_api_key: true,
            config_fields: vec![],
        },
    ]
}

/// Look up one engine's descriptor by registry key.
pub fn descriptor(key: &str) -> Option<ProviderDescriptor> {
    catalog().into_iter().find(|d| d.key == key)
}

/// True when a provider's required config + (if needed) API key are present.
pub fn is_configured(desc: &ProviderDescriptor, api_key: Option<&str>, config: &Value) -> bool {
    if desc.needs_api_key && !api_key.map(|k| !k.trim().is_empty()).unwrap_or(false) {
        return false;
    }
    desc.config_fields.iter().all(|f| {
        !f.required
            || config
                .get(f.key)
                .and_then(|v| v.as_str())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
    })
}

/// True if ≥1 entry in `chain` is a known + configured provider given the
/// stored provider rows. Shared by `search_via_chain` (the "no provider"
/// guard) and the chat extension's attach gate so the two can't drift.
pub fn any_configured_in_chain(chain: &[String], rows: &[WebSearchProviderRow]) -> bool {
    let default_cfg = serde_json::json!({});
    chain.iter().any(|key| {
        let Some(desc) = descriptor(key) else { return false };
        let row = rows.iter().find(|r| r.provider == *key);
        let api_key = row.and_then(|r| r.api_key.as_deref());
        let config = row.map(|r| &r.config).unwrap_or(&default_cfg);
        is_configured(&desc, api_key, config)
    })
}

/// The chat-extension attach gate: web search is available to a chat iff it's
/// enabled AND ≥1 provider in the chain is configured. Pure so the gate logic
/// (the documented silent-failure point) is unit-testable without a DB.
pub fn attach_gate_open(settings: &WebSearchSettings, rows: &[WebSearchProviderRow]) -> bool {
    settings.enabled && any_configured_in_chain(&settings.provider_chain, rows)
}

/// Construct a provider instance from its stored config + key. `timeout_secs`
/// is applied per-request by the provider. Unknown key → typed error.
pub fn build(
    provider: &str,
    config: &Value,
    api_key: Option<String>,
    timeout_secs: u64,
) -> Result<Box<dyn SearchProvider>, AppError> {
    match provider {
        "searxng" => {
            let base_url = config
                .get("base_url")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    AppError::bad_request(
                        "WEB_SEARCH_PROVIDER_UNCONFIGURED",
                        "searxng requires a base_url",
                    )
                })?;
            Ok(Box::new(searxng::SearxngProvider::new(base_url, timeout_secs)?))
        }
        "brave" => {
            let key = api_key.filter(|k| !k.trim().is_empty()).ok_or_else(|| {
                AppError::bad_request(
                    "WEB_SEARCH_PROVIDER_UNCONFIGURED",
                    "brave requires an API key",
                )
            })?;
            Ok(Box::new(brave::BraveProvider::new(key, timeout_secs)?))
        }
        other => Err(AppError::bad_request(
            "WEB_SEARCH_UNKNOWN_PROVIDER",
            format!("unknown search provider: {other}"),
        )),
    }
}

/// Validate that every key in a proposed chain is a known registry key.
/// Used at settings-write time (unknown key → 400-class error).
pub fn validate_chain(chain: &[String]) -> Result<(), AppError> {
    for key in chain {
        if descriptor(key).is_none() {
            return Err(AppError::bad_request(
                "WEB_SEARCH_UNKNOWN_PROVIDER",
                format!("unknown search provider in chain: {key}"),
            ));
        }
    }
    Ok(())
}

/// Validate a provider's config at WRITE time, so a malformed value can't be
/// stored and then mis-reported as `configured` in the catalog (only to fail
/// late at first search). searxng: `base_url`, when present, must parse as an
/// http(s) URL. Other providers have no config to validate today.
pub fn validate_config(provider: &str, config: &Value) -> Result<(), AppError> {
    // A present config must be a JSON object (not a string/number/array) — those
    // can never satisfy any provider's field schema and would store inert junk.
    if !config.is_null() && !config.is_object() {
        return Err(AppError::bad_request(
            "WEB_SEARCH_BAD_CONFIG",
            "config must be a JSON object",
        ));
    }
    if provider == "searxng"
        && let Some(base) = config.get("base_url").and_then(|v| v.as_str())
    {
        let base = base.trim();
        if !base.is_empty() {
            // Match what SearxngProvider::new enforces at search time, so a bad
            // value is rejected at WRITE time rather than mis-reported as
            // configured (the guarantee this fn's doc comment promises).
            match url::Url::parse(base).ok().filter(|u| matches!(u.scheme(), "http" | "https")) {
                None => {
                    return Err(AppError::bad_request(
                        "WEB_SEARCH_BAD_BASE_URL",
                        "searxng base_url must be a valid http(s) URL",
                    ));
                }
                Some(u) if !u.username().is_empty() || u.password().is_some() => {
                    return Err(AppError::bad_request(
                        "WEB_SEARCH_BAD_BASE_URL",
                        "searxng base_url must not embed credentials",
                    ));
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// Run a query through the configured fallback chain. `settings` is passed in
/// by the caller (which has already loaded it + checked `enabled`), so the row
/// is read once per search.
///
/// Iterates `settings.provider_chain` in order: skip entries that aren't a
/// known/configured provider; otherwise call `search`. **Fall back to the next
/// entry ONLY on error** (network/timeout/quota/auth/HTTP-5xx) — a successful
/// result, *including an empty one*, is returned immediately. If no entry is
/// configured, or every configured entry errors, return a typed error.
pub async fn search_via_chain(
    query: &str,
    count: usize,
    settings: &WebSearchSettings,
) -> Result<SearchOutcome, AppError> {
    let timeout = settings.request_timeout_secs.max(1) as u64;
    let rows = Repos.web_search.list_providers().await?;

    // Resolve the chain into built provider instances (skipping unknown /
    // unconfigured entries). The pure walk happens in `run_chain` below so the
    // fallback semantics are unit-testable without a DB.
    let mut candidates: Vec<(String, Box<dyn SearchProvider>)> = Vec::new();
    let mut build_err: Option<AppError> = None;
    for key in &settings.provider_chain {
        let Some(desc) = descriptor(key) else { continue };
        let row = rows.iter().find(|r| &r.provider == key);
        let api_key = row.and_then(|r| r.api_key.clone());
        let config = row.map(|r| r.config.clone()).unwrap_or_else(|| serde_json::json!({}));
        if !is_configured(&desc, api_key.as_deref(), &config) {
            continue;
        }
        match build(key, &config, api_key, timeout) {
            Ok(p) => candidates.push((key.clone(), p)),
            Err(e) => build_err = Some(e),
        }
    }

    if candidates.is_empty() {
        return Err(build_err.unwrap_or_else(|| {
            AppError::bad_request(
                "WEB_SEARCH_NO_PROVIDER",
                "no web search provider is configured; ask an administrator to set one up",
            )
        }));
    }
    run_chain(candidates, query, count).await
}

/// Pure fallback walk over already-resolved providers. Returns the first
/// provider that succeeds (an empty result IS a success — no fallback on
/// empty); advances to the next only on error. If every provider errors,
/// returns the last error. Separated from `search_via_chain` so the fallback
/// semantics are unit-testable with fake providers.
pub async fn run_chain(
    candidates: Vec<(String, Box<dyn SearchProvider>)>,
    query: &str,
    count: usize,
) -> Result<SearchOutcome, AppError> {
    let mut last_err: Option<AppError> = None;
    for (key, provider) in candidates {
        match provider.search(query, count).await {
            Ok(results) => return Ok(SearchOutcome { provider: key, results }),
            Err(e) => {
                tracing::warn!("web_search: provider '{key}' failed ({e}); trying next in chain");
                last_err = Some(e);
            }
        }
    }
    Err(last_err
        .unwrap_or_else(|| AppError::internal_error("web search failed for all configured providers")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn desc(key: &str) -> ProviderDescriptor {
        descriptor(key).unwrap()
    }

    #[test]
    fn catalog_has_searxng_and_brave() {
        let keys: Vec<_> = catalog().into_iter().map(|d| d.key).collect();
        assert!(keys.contains(&"searxng"));
        assert!(keys.contains(&"brave"));
    }

    #[test]
    fn searxng_configured_requires_base_url() {
        let d = desc("searxng");
        assert!(!is_configured(&d, None, &json!({})));
        assert!(!is_configured(&d, None, &json!({ "base_url": "" })));
        assert!(is_configured(&d, None, &json!({ "base_url": "https://s.example" })));
    }

    #[test]
    fn brave_configured_requires_key() {
        let d = desc("brave");
        assert!(!is_configured(&d, None, &json!({})));
        assert!(!is_configured(&d, Some("  "), &json!({})));
        assert!(is_configured(&d, Some("BSA-xxx"), &json!({})));
    }

    #[test]
    fn validate_chain_rejects_unknown() {
        assert!(validate_chain(&["searxng".into(), "brave".into()]).is_ok());
        assert!(validate_chain(&["searxng".into(), "nope".into()]).is_err());
    }

    #[test]
    fn build_unknown_provider_errors() {
        assert!(build("nope", &json!({}), None, 10).is_err());
    }

    #[test]
    fn any_configured_in_chain_gate() {
        let rows = vec![WebSearchProviderRow {
            provider: "searxng".into(),
            api_key: None,
            config: json!({ "base_url": "https://s.example" }),
        }];
        assert!(any_configured_in_chain(&["searxng".into()], &rows));
        // brave needs a key it doesn't have → not configured.
        assert!(!any_configured_in_chain(&["brave".into()], &rows));
        // unknown key + empty chain → false.
        assert!(!any_configured_in_chain(&["nope".into()], &rows));
        assert!(!any_configured_in_chain(&[], &rows));
        // a ready provider anywhere in the chain → true.
        assert!(any_configured_in_chain(&["brave".into(), "searxng".into()], &rows));
    }

    #[test]
    fn validate_config_checks_searxng_base_url() {
        assert!(validate_config("searxng", &json!({ "base_url": "https://s.example" })).is_ok());
        assert!(validate_config("searxng", &json!({ "base_url": "http://10.0.0.5:8080" })).is_ok());
        assert!(validate_config("searxng", &json!({ "base_url": "not a url" })).is_err());
        assert!(validate_config("searxng", &json!({ "base_url": "ftp://x" })).is_err());
        assert!(validate_config("searxng", &json!({})).is_ok()); // absent ok
        assert!(validate_config("searxng", &json!({ "base_url": "" })).is_ok()); // empty ok (just "not configured")
        assert!(validate_config("brave", &json!({ "anything": "x" })).is_ok());
        // A non-object config is rejected for any provider.
        assert!(validate_config("searxng", &json!("a string")).is_err());
        assert!(validate_config("brave", &json!([1, 2, 3])).is_err());
        // Embedded credentials in the base_url are rejected at write time.
        assert!(validate_config("searxng", &json!({ "base_url": "https://user:pass@host" })).is_err());
    }

    fn settings(enabled: bool, chain: &[&str]) -> WebSearchSettings {
        WebSearchSettings {
            enabled,
            provider_chain: chain.iter().map(|s| s.to_string()).collect(),
            max_results: 5,
            fetch_max_bytes: 5_000_000,
            fetch_max_chars: 40_000,
            request_timeout_secs: 20,
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn attach_gate_requires_enabled_and_a_configured_provider() {
        let rows = vec![WebSearchProviderRow {
            provider: "searxng".into(),
            api_key: None,
            config: json!({ "base_url": "https://s.example" }),
        }];
        // enabled + a configured chain provider → open.
        assert!(attach_gate_open(&settings(true, &["searxng"]), &rows));
        // disabled → closed even with a configured provider (regression guard
        // for an inverted enabled check — the documented silent-failure point).
        assert!(!attach_gate_open(&settings(false, &["searxng"]), &rows));
        // enabled but the chained provider isn't configured → closed.
        assert!(!attach_gate_open(&settings(true, &["brave"]), &rows));
        // enabled but empty chain → closed.
        assert!(!attach_gate_open(&settings(true, &[]), &rows));
    }

    // ── Fallback-chain semantics (run_chain, with fake providers) ──────────

    fn hit(url: &str) -> SearchHit {
        SearchHit {
            title: "t".into(),
            url: url.into(),
            snippet: "s".into(),
        }
    }

    struct FakeOk(Vec<SearchHit>);
    #[async_trait::async_trait]
    impl SearchProvider for FakeOk {
        async fn search(&self, _q: &str, _n: usize) -> Result<Vec<SearchHit>, AppError> {
            Ok(self.0.clone())
        }
    }

    struct FakeErr;
    #[async_trait::async_trait]
    impl SearchProvider for FakeErr {
        async fn search(&self, _q: &str, _n: usize) -> Result<Vec<SearchHit>, AppError> {
            Err(AppError::internal_error("boom"))
        }
    }

    #[tokio::test]
    async fn chain_returns_first_success() {
        let c: Vec<(String, Box<dyn SearchProvider>)> = vec![
            ("a".into(), Box::new(FakeOk(vec![hit("https://a")]))),
            ("b".into(), Box::new(FakeOk(vec![hit("https://b")]))),
        ];
        let out = run_chain(c, "q", 5).await.unwrap();
        assert_eq!(out.provider, "a");
        assert_eq!(out.results[0].url, "https://a");
    }

    #[tokio::test]
    async fn chain_falls_back_on_error() {
        let c: Vec<(String, Box<dyn SearchProvider>)> = vec![
            ("a".into(), Box::new(FakeErr)),
            ("b".into(), Box::new(FakeOk(vec![hit("https://b")]))),
        ];
        let out = run_chain(c, "q", 5).await.unwrap();
        assert_eq!(out.provider, "b");
    }

    #[tokio::test]
    async fn chain_empty_result_is_final_no_fallback() {
        // First provider succeeds with ZERO hits → returned as-is; the second
        // (which has hits) must NOT be consulted. provider == "a" proves it.
        let c: Vec<(String, Box<dyn SearchProvider>)> = vec![
            ("a".into(), Box::new(FakeOk(vec![]))),
            ("b".into(), Box::new(FakeOk(vec![hit("https://b")]))),
        ];
        let out = run_chain(c, "q", 5).await.unwrap();
        assert_eq!(out.provider, "a");
        assert!(out.results.is_empty());
    }

    #[tokio::test]
    async fn chain_all_errors_returns_err() {
        let c: Vec<(String, Box<dyn SearchProvider>)> = vec![
            ("a".into(), Box::new(FakeErr)),
            ("b".into(), Box::new(FakeErr)),
        ];
        assert!(run_chain(c, "q", 5).await.is_err());
    }
}
