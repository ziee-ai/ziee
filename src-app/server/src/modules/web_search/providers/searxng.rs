//! SearXNG search provider.
//!
//! SearXNG instances are **admin-configured + trusted**, so the base URL is
//! allowed to be private/loopback (a self-hosted SearXNG on a LAN is the common
//! deployment) — UNLIKE the untrusted page-fetch path, which is locked to
//! public addresses.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{SearchHit, SearchProvider};
use crate::common::AppError;
use crate::utils::url_validator::{OutboundUrlPolicy, build_validated_client, validate_outbound_url};

/// Trusted-endpoint policy: http+https, private + loopback allowed (the admin
/// chose this host). Still rejects bad schemes / embedded credentials.
const SEARXNG_POLICY: OutboundUrlPolicy = OutboundUrlPolicy {
    allow_schemes: &["http", "https"],
    allow_localhost: true,
    allow_private: true,
    // Admin-configured trusted host; preserve prior behavior (link-local was
    // permitted alongside private before the flag was split out).
    allow_link_local: true,
};

pub struct SearxngProvider {
    search_url: String,
    client: reqwest::Client,
    timeout: Duration,
}

impl SearxngProvider {
    pub fn new(base_url: String, timeout_secs: u64) -> Result<Self, AppError> {
        let trimmed = base_url.trim().trim_end_matches('/').to_string();
        // Fail fast on a malformed/unsafe base URL (scheme/parse). The redirect
        // policy of the built client re-validates each hop under the same rule.
        validate_outbound_url(&trimmed, &SEARXNG_POLICY).map_err(|e| {
            AppError::bad_request(
                "WEB_SEARCH_BAD_BASE_URL",
                format!("searxng base_url failed validation: {e}"),
            )
        })?;
        let client = build_validated_client(SEARXNG_POLICY)
            .map_err(|e| AppError::internal_error(format!("failed to build http client: {e}")))?;
        Ok(Self {
            search_url: format!("{trimmed}/search"),
            client,
            timeout: Duration::from_secs(timeout_secs),
        })
    }
}

#[derive(Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(Deserialize)]
struct SearxngResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    async fn search(&self, query: &str, count: usize) -> Result<Vec<SearchHit>, AppError> {
        let resp = self
            .client
            .get(&self.search_url)
            .query(&[("q", query), ("format", "json")])
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| {
                // The reqwest error embeds the request URL — i.e. the
                // admin-configured (possibly private/LAN/loopback) SearXNG host.
                // Keep it server-side; the model-visible error must not leak the
                // internal topology (trust-boundary separation).
                tracing::warn!("searxng request failed: {e}");
                AppError::internal_error("searxng request failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "searxng returned HTTP {}",
                resp.status()
            )));
        }
        let parsed: SearxngResponse =
            super::read_json_capped(resp, super::MAX_SEARCH_BODY_BYTES).await?;
        Ok(map_results(parsed, count))
    }
}

/// Map a parsed SearXNG response to ranked hits: drop result rows with an empty
/// URL, then cap at `count`. Pure so the filter/cap branches are unit-testable
/// without a live HTTP server.
fn map_results(resp: SearxngResponse, count: usize) -> Vec<SearchHit> {
    resp.results
        .into_iter()
        .filter(|r| !r.url.is_empty())
        .take(count)
        .map(|r| SearchHit {
            title: r.title,
            url: r.url,
            snippet: r.content,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(rows: &[(&str, &str, &str)]) -> SearxngResponse {
        SearxngResponse {
            results: rows
                .iter()
                .map(|(t, u, c)| SearxngResult {
                    title: t.to_string(),
                    url: u.to_string(),
                    content: c.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn map_results_drops_empty_url_and_caps_count() {
        let r = resp(&[
            ("A", "https://a", "sa"),
            ("B", "", "sb"), // empty url → dropped
            ("C", "https://c", "sc"),
            ("D", "https://d", "sd"),
        ]);
        let hits = map_results(r, 2);
        assert_eq!(hits.len(), 2, "count cap not applied");
        assert_eq!(hits[0].url, "https://a");
        // B was dropped, so the second kept hit is C (not the empty-url B).
        assert_eq!(hits[1].url, "https://c");
        assert_eq!(hits[0].snippet, "sa");
    }
}
