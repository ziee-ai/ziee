//! CORE connector — open-access full text + repositories / theses / grey
//! literature, JSON REST (v3). Requires a free API key (bearer token).

use async_trait::async_trait;
use serde::Deserialize;

use super::{LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_json_capped};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

const ENDPOINT: &str = "https://api.core.ac.uk/v3/search/works";

pub struct CoreConnector {
    client: reqwest::Client,
    api_key: String,
}

impl CoreConnector {
    pub fn new(api_key: String) -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
            api_key,
        })
    }
}

#[derive(Deserialize)]
struct CoreResponse {
    #[serde(default)]
    results: Vec<CoreWork>,
}

#[derive(Deserialize)]
struct CoreWork {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    title: Option<String>,
    #[serde(rename = "abstract", default)]
    abstract_text: Option<String>,
    #[serde(default)]
    authors: Vec<CoreAuthor>,
    #[serde(rename = "yearPublished", default)]
    year_published: Option<i32>,
    #[serde(default)]
    doi: Option<String>,
    #[serde(rename = "downloadUrl", default)]
    download_url: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
}

#[derive(Deserialize)]
struct CoreAuthor {
    #[serde(default)]
    name: Option<String>,
}

fn id_string(v: &Option<serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn map_results(resp: CoreResponse) -> Vec<LitRecord> {
    resp.results
        .into_iter()
        .filter_map(|w| {
            let title = w.title.filter(|t| !t.trim().is_empty())?;
            let doi = w.doi.filter(|d| !d.is_empty());
            let id = id_string(&w.id);
            let url = w
                .download_url
                .filter(|u| !u.is_empty())
                .or_else(|| doi.as_deref().map(|d| format!("https://doi.org/{d}")));
            Some(LitRecord {
                doi: doi.clone(),
                pmid: None,
                title,
                abstract_text: w.abstract_text.filter(|a| !a.trim().is_empty()),
                authors: w.authors.into_iter().filter_map(|a| a.name).collect(),
                year: w.year_published,
                venue: w.publisher.filter(|p| !p.is_empty()),
                url,
                source: "core".into(),
                source_ids: vec![format!("core:{id}")],
                cited_by_count: None,
                is_preprint: false,
                relevance: 0.0,
            })
        })
        .collect()
}

#[async_trait]
impl LitConnector for CoreConnector {
    fn key(&self) -> &'static str {
        "core"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        let mut q = query.to_string();
        if let Some(f) = opts.year_from {
            q.push_str(&format!(" AND yearPublished>={f}"));
        }
        if let Some(t) = opts.year_to {
            q.push_str(&format!(" AND yearPublished<={t}"));
        }
        let limit = opts.limit.clamp(1, 100).to_string();
        let resp = self
            .client
            .get(endpoint(ENDPOINT, "LIT_SEARCH_CORE_ENDPOINT"))
            .query(&[("q", q.as_str()), ("limit", limit.as_str())])
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("core request failed: {e}");
                AppError::internal_error("core request failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "core returned HTTP {}",
                resp.status()
            )));
        }
        let parsed: CoreResponse = read_json_capped(resp, MAX_BODY_BYTES).await?;
        Ok(map_results(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_work() {
        let json = serde_json::json!({
            "results": [{
                "id": 12345,
                "title": "An OA paper",
                "abstract": "abstract text",
                "authors": [{ "name": "Jane Smith" }],
                "yearPublished": 2019,
                "doi": "10.1/core",
                "downloadUrl": "https://core.ac.uk/download/12345.pdf",
                "publisher": "Univ Press"
            }]
        });
        let resp: CoreResponse = serde_json::from_value(json).unwrap();
        let recs = map_results(resp);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/core"));
        assert_eq!(recs[0].year, Some(2019));
        assert_eq!(recs[0].url.as_deref(), Some("https://core.ac.uk/download/12345.pdf"));
        assert_eq!(recs[0].source_ids, vec!["core:12345"]);
    }
}
