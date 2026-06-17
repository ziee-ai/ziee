//! Brave Search API provider (BYO API key).
//!
//! Brave's endpoint is a fixed public HTTPS host, so the strictest SSRF policy
//! applies. The deployment API key is sent in the `X-Subscription-Token` header.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{SearchHit, SearchProvider};
use crate::common::AppError;
use crate::utils::url_validator::{OutboundUrlPolicy, build_validated_client};

const BRAVE_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Resolve the Brave endpoint + SSRF policy. In DEBUG builds ONLY, the endpoint
/// may be overridden to a loopback mock via `WEB_SEARCH_BRAVE_ENDPOINT`, which
/// also relaxes the policy to `DEV_LOCAL` so the mock is reachable. Compiled out
/// of release builds via `cfg!(debug_assertions)` (same seam pattern as
/// `WEB_SEARCH_FETCH_ALLOW_LOOPBACK`); it cannot be set in production.
fn brave_endpoint_and_policy() -> (String, OutboundUrlPolicy) {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("WEB_SEARCH_BRAVE_ENDPOINT") {
        return (url, OutboundUrlPolicy::DEV_LOCAL);
    }
    (BRAVE_ENDPOINT.to_string(), OutboundUrlPolicy::STRICT)
}

pub struct BraveProvider {
    api_key: String,
    client: reqwest::Client,
    timeout: Duration,
    endpoint: String,
}

impl BraveProvider {
    pub fn new(api_key: String, timeout_secs: u64) -> Result<Self, AppError> {
        let (endpoint, policy) = brave_endpoint_and_policy();
        let client = build_validated_client(policy)
            .map_err(|e| AppError::internal_error(format!("failed to build http client: {e}")))?;
        Ok(Self {
            api_key,
            client,
            timeout: Duration::from_secs(timeout_secs),
            endpoint,
        })
    }
}

#[derive(Deserialize)]
struct BraveResponse {
    web: Option<BraveWeb>,
}

#[derive(Deserialize)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Deserialize)]
struct BraveResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
}

#[async_trait]
impl SearchProvider for BraveProvider {
    async fn search(&self, query: &str, count: usize) -> Result<Vec<SearchHit>, AppError> {
        let count_str = count.clamp(1, 20).to_string();
        let resp = self
            .client
            .get(&self.endpoint)
            .query(&[("q", query), ("count", count_str.as_str())])
            .header("X-Subscription-Token", self.api_key.as_str())
            .header("Accept", "application/json")
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| {
                // Keep transport detail server-side (mirrors searxng.rs) — the
                // model-visible error stays generic.
                tracing::warn!("brave request failed: {e}");
                AppError::internal_error("brave request failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "brave returned HTTP {}",
                resp.status()
            )));
        }
        let parsed: BraveResponse =
            super::read_json_capped(resp, super::MAX_SEARCH_BODY_BYTES).await?;
        Ok(map_results(parsed, count))
    }
}

/// Map a parsed Brave response to ranked hits: drop empty-URL rows, cap at
/// `count`, remap `description` → snippet. Pure so the mapping branches are
/// unit-testable without a live HTTP server (mirrors searxng::map_results).
fn map_results(resp: BraveResponse, count: usize) -> Vec<SearchHit> {
    resp.web
        .map(|w| w.results)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| !r.url.is_empty())
        .take(count)
        .map(|r| SearchHit {
            title: r.title,
            url: r.url,
            snippet: r.description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(rows: &[(&str, &str, &str)]) -> BraveResponse {
        BraveResponse {
            web: Some(BraveWeb {
                results: rows
                    .iter()
                    .map(|(t, u, d)| BraveResult {
                        title: t.to_string(),
                        url: u.to_string(),
                        description: d.to_string(),
                    })
                    .collect(),
            }),
        }
    }

    #[test]
    fn map_results_handles_missing_web_drops_empty_url_caps_and_remaps_description() {
        // No `web` object → empty.
        assert!(map_results(BraveResponse { web: None }, 5).is_empty());

        let hits = map_results(
            resp(&[
                ("A", "https://a", "da"),
                ("B", "", "db"), // empty url → dropped
                ("C", "https://c", "dc"),
                ("D", "https://d", "dd"),
            ]),
            2,
        );
        assert_eq!(hits.len(), 2, "count cap not applied");
        assert_eq!(hits[0].url, "https://a");
        assert_eq!(hits[1].url, "https://c"); // B dropped
        assert_eq!(hits[0].snippet, "da"); // description → snippet
    }
}
