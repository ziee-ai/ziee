//! Semantic Scholar Graph API connector — broad cross-domain index, JSON.
//! Works keyless (shared pool — may 429) or with an optional free `x-api-key`.
//! `externalIds` carries DOI + PMID. Honors a single `429`/`Retry-After` backoff.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_json_capped};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

const ENDPOINT: &str = "https://api.semanticscholar.org/graph/v1/paper/search";
const FIELDS: &str =
    "title,abstract,year,venue,externalIds,citationCount,authors,openAccessPdf,publicationTypes";

pub struct SemanticScholarConnector {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl SemanticScholarConnector {
    pub fn new(api_key: Option<String>) -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
            api_key: api_key.filter(|k| !k.trim().is_empty()),
        })
    }
}

#[derive(Deserialize)]
struct S2Response {
    #[serde(default)]
    data: Vec<S2Paper>,
}

#[derive(Deserialize)]
struct S2Paper {
    #[serde(rename = "paperId", default)]
    paper_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    r#abstract: Option<String>,
    #[serde(default)]
    year: Option<i32>,
    #[serde(default)]
    venue: Option<String>,
    #[serde(rename = "externalIds", default)]
    external_ids: Option<S2ExternalIds>,
    #[serde(rename = "citationCount", default)]
    citation_count: Option<i64>,
    #[serde(default)]
    authors: Vec<S2Author>,
    #[serde(rename = "openAccessPdf", default)]
    open_access_pdf: Option<S2Pdf>,
    #[serde(rename = "publicationTypes", default)]
    publication_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct S2ExternalIds {
    #[serde(rename = "DOI", default)]
    doi: Option<String>,
    #[serde(rename = "PubMed", default)]
    pubmed: Option<String>,
    #[serde(rename = "ArXiv", default)]
    arxiv: Option<String>,
}

#[derive(Deserialize)]
struct S2Author {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct S2Pdf {
    #[serde(default)]
    url: Option<String>,
}

fn map_results(resp: S2Response) -> Vec<LitRecord> {
    resp.data
        .into_iter()
        .filter_map(|p| {
            let title = p.title.filter(|t| !t.trim().is_empty())?;
            let ids = p.external_ids;
            let doi = ids.as_ref().and_then(|i| i.doi.clone()).filter(|d| !d.is_empty());
            let pmid = ids.as_ref().and_then(|i| i.pubmed.clone()).filter(|p| !p.is_empty());
            // Emptiness-filtered like doi/pmid: an `"ArXiv": ""` must NOT flag a
            // preprint (it would bias dedup's is_preprint merge + the digest label).
            let is_arxiv = ids
                .as_ref()
                .and_then(|i| i.arxiv.as_deref())
                .map(|a| !a.trim().is_empty())
                .unwrap_or(false);
            let is_preprint = is_arxiv
                || p.publication_types
                    .as_ref()
                    .map(|v| v.iter().any(|t| t.eq_ignore_ascii_case("Preprint")))
                    .unwrap_or(false);
            let authors = p.authors.into_iter().filter_map(|a| a.name).collect();
            let url = p
                .open_access_pdf
                .and_then(|pdf| pdf.url)
                .filter(|u| !u.is_empty())
                .or_else(|| doi.as_deref().map(|d| format!("https://doi.org/{d}")));
            Some(LitRecord {
                doi,
                pmid,
                title,
                abstract_text: p.r#abstract.filter(|a| !a.is_empty()),
                authors,
                year: p.year,
                venue: p.venue.filter(|v| !v.is_empty()),
                url,
                source: "semanticscholar".into(),
                source_ids: vec![format!("semanticscholar:{}", p.paper_id)],
                cited_by_count: p.citation_count,
                is_preprint,
                relevance: 0.0,
            })
        })
        .collect()
}

#[async_trait]
impl LitConnector for SemanticScholarConnector {
    fn key(&self) -> &'static str {
        "semanticscholar"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        let limit = opts.limit.clamp(1, 100).to_string();
        let mut params: Vec<(&str, String)> = vec![
            ("query", query.to_string()),
            ("limit", limit),
            ("fields", FIELDS.to_string()),
        ];
        match (opts.year_from, opts.year_to) {
            (Some(f), Some(t)) => params.push(("year", format!("{f}-{t}"))),
            (Some(f), None) => params.push(("year", format!("{f}-"))),
            (None, Some(t)) => params.push(("year", format!("-{t}"))),
            (None, None) => {}
        }

        // One retry on 429 (the keyless shared pool rate-limits aggressively).
        for attempt in 0..2 {
            let mut req = self
                .client
                .get(endpoint(ENDPOINT, "LIT_SEARCH_S2_ENDPOINT"))
                .query(&params)
                .header("Accept", "application/json")
                .timeout(opts.timeout);
            if let Some(k) = &self.api_key {
                req = req.header("x-api-key", k.as_str());
            }
            let resp = req.send().await.map_err(|e| {
                tracing::warn!("semanticscholar request failed: {e}");
                AppError::internal_error("semanticscholar request failed")
            })?;
            if resp.status().as_u16() == 429 && attempt == 0 {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2)
                    .min(5);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if !resp.status().is_success() {
                let status = resp.status();
                // A 429 here is the SECOND (post-retry) hit — surface the
                // add-a-key hint rather than the bare status (the retry on
                // attempt 0 `continue`d above).
                if status.as_u16() == 429 {
                    return Err(AppError::internal_error(
                        "semanticscholar rate-limited (429); add an API key for a dedicated rate",
                    ));
                }
                return Err(AppError::internal_error(format!(
                    "semanticscholar returned HTTP {status}"
                )));
            }
            let parsed: S2Response = read_json_capped(resp, MAX_BODY_BYTES).await?;
            return Ok(map_results(parsed));
        }
        // The `0..2` loop returns on every path; this is unreachable but the
        // compiler can't prove it (runtime-bounded loop).
        unreachable!("semanticscholar retry loop always returns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_external_ids_to_doi_pmid_and_detects_arxiv_preprint() {
        let json = serde_json::json!({
            "data": [{
                "paperId": "abc",
                "title": "T",
                "abstract": "A",
                "year": 2022,
                "venue": "V",
                "externalIds": { "DOI": "10.1/x", "PubMed": "999", "ArXiv": "2201.00001" },
                "citationCount": 5,
                "authors": [{ "name": "Jane Smith" }]
            }]
        });
        let resp: S2Response = serde_json::from_value(json).unwrap();
        let recs = map_results(resp);
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/x"));
        assert_eq!(recs[0].pmid.as_deref(), Some("999"));
        assert!(recs[0].is_preprint, "ArXiv id present → preprint");
        assert_eq!(recs[0].authors, vec!["Jane Smith"]);
        assert_eq!(recs[0].source_ids, vec!["semanticscholar:abc"]);
    }
}
