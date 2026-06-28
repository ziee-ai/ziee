//! Connector registry + concurrent UNION aggregation.
//!
//! The *set* of supported sources lives HERE (`catalog()`), not in the DB — the
//! `lit_search_connectors` table only stores `{api_key, config}` keyed by a
//! registry key. Adding a source = implement [`LitConnector`], add a
//! [`ConnectorDescriptor`] to `catalog()`, and a `build()` arm — no migration, no
//! frontend change (the admin UI renders from the descriptor catalog).
//!
//! Aggregation is a **UNION** across the enabled connectors (not a fallback
//! chain): every connector runs concurrently, a failing one contributes zero
//! records and is recorded in `degraded_sources`, and the rest still return.

pub mod arxiv;
pub mod core;
pub mod crossref;
pub mod europepmc;
pub mod pubmed;
pub mod semanticscholar;

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Datelike;
use serde_json::Value;

use crate::common::AppError;
use crate::core::Repos;
use crate::utils::url_validator::{OutboundUrlPolicy, build_validated_client};

// Shared capped-body readers (the security control lifted to `utils/http_body`).
// Re-exported so every connector's `use super::{… read_json_capped}` keeps working.
pub use crate::utils::http_body::{read_bytes_capped, read_json_capped, read_text_capped};

use super::models::{AggregateResult, LitRecord, LitSearchSettings};
use super::{completeness, dedup, ranking};

/// Hard cap on a connector's response body. Literature responses (esp. a
/// many-record JSON page) are larger than web-search results.
pub const MAX_BODY_BYTES: u64 = 24 * 1024 * 1024;

/// Outbound SSRF policy for the HTTP clients. For the SEARCH connectors this is
/// defense-in-depth — they hit FIXED public hosts with server-built URLs (no
/// model/admin-supplied base URL). But the SAME client is used by the full-text
/// resolver (`fulltext/resolvers.rs`) to fetch OA-PDF URLs taken VERBATIM from
/// Unpaywall API responses (third-party-controlled hosts) — there
/// `PUBLIC_HTTP_OR_HTTPS` is a PRIMARY SSRF boundary (its `GuardingResolver`
/// blocks loopback/RFC1918/IMDS and re-validates redirects). Do NOT weaken it.
/// A DEBUG-only env seam relaxes it to loopback for test mocks; it is compiled
/// out of release builds via `cfg!(debug_assertions)`.
pub fn connector_policy() -> OutboundUrlPolicy {
    #[cfg(debug_assertions)]
    if std::env::var("LIT_SEARCH_ALLOW_LOOPBACK").is_ok() {
        return OutboundUrlPolicy::DEV_LOCAL;
    }
    OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
}

/// A reqwest client whose DNS resolver + redirect policy enforce the SSRF policy.
pub fn build_client() -> Result<reqwest::Client, AppError> {
    build_validated_client(connector_policy())
        .map_err(|e| AppError::internal_error(format!("failed to build http client: {e}")))
}

/// Debug-only per-connector endpoint override (the testability seam, mirroring
/// web_search's `WEB_SEARCH_BRAVE_ENDPOINT`). Returns the value of
/// `LIT_SEARCH_<KEY>_ENDPOINT` when set, else the production `default`. Pair with
/// `LIT_SEARCH_ALLOW_LOOPBACK=1` so the SSRF policy permits a `127.0.0.1` mock.
/// **Compiled out of release builds** via `cfg!(debug_assertions)` — production
/// always hits the fixed public host.
pub fn endpoint(default: &str, env_key: &str) -> String {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var(env_key)
        && !url.trim().is_empty()
    {
        return url;
    }
    let _ = env_key;
    default.to_string()
}

/// Per-search options handed to each connector.
#[derive(Clone, Copy, Debug)]
pub struct SearchOpts {
    pub limit: usize,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub timeout: Duration,
}

#[async_trait]
pub trait LitConnector: Send + Sync {
    #[allow(dead_code)]
    fn key(&self) -> &'static str;
    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError>;
}

// ── Code-owned descriptor catalog (drives the admin UI) ─────────────────────

#[derive(Clone)]
pub struct ConfigField {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub placeholder: &'static str,
    pub help: Option<&'static str>,
    pub docs_url: Option<&'static str>,
}

#[derive(Clone)]
pub struct KeyField {
    pub required: bool,
    pub label: &'static str,
    pub help: Option<&'static str>,
    pub docs_url: Option<&'static str>,
}

#[derive(Clone)]
pub struct ConnectorDescriptor {
    pub key: &'static str,
    pub display_name: &'static str,
    pub keyless_note: &'static str,
    pub key_field: Option<KeyField>,
    pub config_fields: Vec<ConfigField>,
}

/// The connectors this build supports. Append here to add a source.
pub fn catalog() -> Vec<ConnectorDescriptor> {
    vec![
        ConnectorDescriptor {
            key: "europepmc",
            display_name: "Europe PMC",
            keyless_note: "Works without a key — biomedical literature + preprints.",
            key_field: None,
            config_fields: vec![],
        },
        ConnectorDescriptor {
            key: "crossref",
            display_name: "Crossref",
            keyless_note: "Works without a key; add a contact email to join the polite pool (higher rate limit).",
            key_field: Some(KeyField {
                required: false,
                label: "Crossref Plus token",
                help: Some("Optional — Crossref Plus subscribers get higher rate limits."),
                docs_url: Some("https://www.crossref.org/documentation/metadata-plus/"),
            }),
            config_fields: vec![ConfigField {
                key: "mailto",
                label: "Contact email (polite pool)",
                required: false,
                placeholder: "you@example.org",
                help: Some(
                    "Joins Crossref's polite pool (~10 req/s vs ~5 anonymous) AND \
                     enables Unpaywall, the DOI→open-access-PDF resolver used by \
                     fetch_paper_fulltext — without it, DOI-only papers can't resolve full text.",
                ),
                docs_url: None,
            }],
        },
        ConnectorDescriptor {
            key: "semanticscholar",
            display_name: "Semantic Scholar",
            keyless_note: "Works without a key (shared pool — may rate-limit); add a free key for a dedicated rate.",
            key_field: Some(KeyField {
                required: false,
                label: "Semantic Scholar API key",
                help: Some("Free; sent as x-api-key. Recommended for reliable throughput."),
                docs_url: Some("https://www.semanticscholar.org/product/api#api-key"),
            }),
            config_fields: vec![],
        },
        ConnectorDescriptor {
            key: "pubmed",
            display_name: "PubMed (NCBI E-utilities)",
            keyless_note: "Works without a key (3 req/s); add a free NCBI key for 10 req/s.",
            key_field: Some(KeyField {
                required: false,
                label: "NCBI API key",
                help: Some("Free — raises the rate limit from 3 to 10 req/s."),
                docs_url: Some("https://www.ncbi.nlm.nih.gov/account/settings/"),
            }),
            config_fields: vec![ConfigField {
                key: "mailto",
                label: "Contact email",
                required: false,
                placeholder: "you@example.org",
                help: Some("NCBI asks API users to identify themselves."),
                docs_url: None,
            }],
        },
        ConnectorDescriptor {
            key: "arxiv",
            display_name: "arXiv",
            keyless_note: "Works without a key — CS / methods / quant-bio preprints.",
            key_field: None,
            config_fields: vec![],
        },
        ConnectorDescriptor {
            key: "core",
            display_name: "CORE (open-access full text)",
            keyless_note: "Requires a free CORE API key.",
            key_field: Some(KeyField {
                required: true,
                label: "CORE API key",
                help: Some("Free — register at core.ac.uk; sent as a bearer token."),
                docs_url: Some("https://core.ac.uk/services/api"),
            }),
            config_fields: vec![],
        },
    ]
}

pub fn descriptor(key: &str) -> Option<ConnectorDescriptor> {
    catalog().into_iter().find(|d| d.key == key)
}

/// True when a connector's required key + required config fields are present.
/// Keyless sources (no required key/config) are always configured.
pub fn is_configured(desc: &ConnectorDescriptor, api_key: Option<&str>, config: &Value) -> bool {
    if let Some(kf) = &desc.key_field
        && kf.required
        && !api_key.map(|k| !k.trim().is_empty()).unwrap_or(false)
    {
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

/// Validate that every key is a known catalog key (settings write-time).
pub fn validate_connectors(connectors: &[String]) -> Result<(), AppError> {
    for key in connectors {
        if descriptor(key).is_none() {
            return Err(AppError::bad_request(
                "LIT_SEARCH_UNKNOWN_CONNECTOR",
                format!("unknown connector in set: {key}"),
            ));
        }
    }
    Ok(())
}

/// Validate a connector's `config` object at write time (parity with
/// web_search's config validation): every key must be a declared `config_field`
/// for that connector — reject unknown keys rather than silently storing them.
pub fn validate_config(connector: &str, config: &Value) -> Result<(), AppError> {
    let Some(desc) = descriptor(connector) else {
        return Ok(()); // unknown connector is rejected separately by the caller
    };
    let Some(obj) = config.as_object() else {
        return Err(AppError::bad_request(
            "LIT_SEARCH_INVALID_CONFIG",
            "config must be a JSON object",
        ));
    };
    for k in obj.keys() {
        if !desc.config_fields.iter().any(|f| f.key == k) {
            return Err(AppError::bad_request(
                "LIT_SEARCH_INVALID_CONFIG",
                format!("unknown config field '{k}' for connector '{connector}'"),
            ));
        }
    }
    Ok(())
}

/// The chat-extension attach gate: lit_search is available iff enabled. All
/// default sources work keyless (CORE self-skips when enabled-but-unkeyed), so
/// there is no connector-config gate. Pure so it's unit-testable without a DB.
pub fn attach_gate_open(settings: &LitSearchSettings) -> bool {
    settings.enabled
}

/// Construct a connector instance from its registry key + stored config/key.
pub fn build(
    key: &str,
    api_key: Option<String>,
    config: &Value,
) -> Result<Box<dyn LitConnector>, AppError> {
    let mailto = config
        .get("mailto")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    match key {
        "europepmc" => Ok(Box::new(europepmc::EuropePmcConnector::new()?)),
        "crossref" => Ok(Box::new(crossref::CrossrefConnector::new(mailto, api_key)?)),
        "semanticscholar" => Ok(Box::new(semanticscholar::SemanticScholarConnector::new(api_key)?)),
        "pubmed" => Ok(Box::new(pubmed::PubmedConnector::new(mailto, api_key)?)),
        "arxiv" => Ok(Box::new(arxiv::ArxivConnector::new()?)),
        "core" => {
            let key = api_key.filter(|k| !k.trim().is_empty()).ok_or_else(|| {
                AppError::bad_request("LIT_SEARCH_CONNECTOR_UNCONFIGURED", "core requires an API key")
            })?;
            Ok(Box::new(core::CoreConnector::new(key)?))
        }
        other => Err(AppError::bad_request(
            "LIT_SEARCH_UNKNOWN_CONNECTOR",
            format!("unknown connector: {other}"),
        )),
    }
}

/// Run a query across all enabled+configured connectors concurrently (UNION),
/// then dedup → rank → (optionally) estimate completeness.
pub async fn aggregate_search(
    query: &str,
    year_from: Option<i32>,
    year_to: Option<i32>,
    settings: &LitSearchSettings,
) -> Result<AggregateResult, AppError> {
    let timeout = Duration::from_secs(settings.request_timeout_secs.max(1) as u64);
    // 100 = the per-connector page-size ceiling (every connector clamps to 100);
    // a higher value fetches nothing extra. Matches the handler validation + the
    // migration CHECK.
    let per_source = settings.per_source_limit.clamp(1, 100) as usize;
    let opts = SearchOpts {
        limit: per_source,
        year_from,
        year_to,
        timeout,
    };

    let rows = Repos.lit_search.list_connectors().await?;

    // Resolve enabled connectors into built instances (skip unknown/unconfigured;
    // an enabled-but-unconfigured source — e.g. CORE without a key — is recorded
    // as degraded rather than silently dropped).
    let mut built: Vec<(String, Box<dyn LitConnector>)> = Vec::new();
    let mut degraded: Vec<String> = Vec::new();
    let mut seen_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for key in &settings.enabled_connectors {
        // De-dup: a duplicate key in enabled_connectors must not spawn the
        // connector (and its upstream request) twice.
        if !seen_keys.insert(key.as_str()) {
            continue;
        }
        let Some(desc) = descriptor(key) else { continue };
        let row = rows.iter().find(|r| &r.connector == key);
        let api_key = row.and_then(|r| r.api_key.clone());
        let config = row.map(|r| r.config.clone()).unwrap_or_else(|| serde_json::json!({}));
        if !is_configured(&desc, api_key.as_deref(), &config) {
            degraded.push(key.clone());
            continue;
        }
        match build(key, api_key, &config) {
            Ok(c) => built.push((key.clone(), c)),
            Err(_) => degraded.push(key.clone()),
        }
    }

    // Fan out concurrently — UNION semantics.
    let futures = built.iter().map(|(key, c)| {
        let q = query.to_string();
        async move { (key.clone(), c.search(&q, opts).await) }
    });
    let results = futures_util::future::join_all(futures).await;

    let mut identified: BTreeMap<String, usize> = BTreeMap::new();
    let mut all: Vec<LitRecord> = Vec::new();
    for (key, res) in results {
        match res {
            Ok(recs) => {
                identified.insert(key.clone(), recs.len());
                all.extend(recs);
            }
            Err(e) => {
                tracing::warn!("lit_search: connector '{key}' failed: {e}");
                identified.insert(key.clone(), 0);
                degraded.push(key);
            }
        }
    }

    let mut records = dedup::merge_by_doi(all);
    let current_year = chrono::Utc::now().year();
    ranking::rank(&mut records, query, current_year);
    let after_dedup = records.len();

    // Cap to max_results AFTER ranking so the best survive.
    let max = settings.max_results.clamp(1, 200) as usize;
    records.truncate(max);

    let completeness = if settings.completeness_estimate_enabled {
        Some(completeness::estimate(&records, &identified))
    } else {
        None
    };

    degraded.sort();
    degraded.dedup();

    Ok(AggregateResult {
        query: query.to_string(),
        records,
        identified,
        after_dedup,
        degraded_sources: degraded,
        completeness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(enabled: bool) -> LitSearchSettings {
        LitSearchSettings {
            enabled,
            enabled_connectors: vec!["europepmc".into(), "crossref".into()],
            max_results: 25,
            per_source_limit: 50,
            request_timeout_secs: 30,
            completeness_estimate_enabled: true,
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn catalog_has_all_six_sources() {
        let keys: Vec<_> = catalog().into_iter().map(|d| d.key).collect();
        for k in ["europepmc", "crossref", "semanticscholar", "pubmed", "arxiv", "core"] {
            assert!(keys.contains(&k), "missing connector {k}");
        }
    }

    #[test]
    fn validate_connectors_rejects_unknown() {
        assert!(validate_connectors(&["europepmc".into(), "crossref".into()]).is_ok());
        assert!(validate_connectors(&["nope".into()]).is_err());
    }

    #[test]
    fn keyless_sources_are_always_configured() {
        let epmc = descriptor("europepmc").unwrap();
        assert!(is_configured(&epmc, None, &serde_json::json!({})));
        let arxiv = descriptor("arxiv").unwrap();
        assert!(is_configured(&arxiv, None, &serde_json::json!({})));
    }

    #[test]
    fn core_requires_a_key() {
        let core = descriptor("core").unwrap();
        assert!(!is_configured(&core, None, &serde_json::json!({})));
        assert!(!is_configured(&core, Some("  "), &serde_json::json!({})));
        assert!(is_configured(&core, Some("KEY"), &serde_json::json!({})));
    }

    #[test]
    fn attach_gate_is_just_enabled() {
        assert!(attach_gate_open(&settings(true)));
        assert!(!attach_gate_open(&settings(false)));
    }
}
