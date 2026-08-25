//! Europe PMC connector — biomedical literature + preprints, JSON REST.
//! Endpoint: the public `webservices/rest/search` (Apache Solr) with
//! `resultType=core` for abstracts. No key. Preprints have `source == "PPR"`.

use async_trait::async_trait;
use serde::Deserialize;

use super::{LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_json_capped};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

const ENDPOINT: &str = "https://www.ebi.ac.uk/europepmc/webservices/rest/search";

pub struct EuropePmcConnector {
    client: reqwest::Client,
}

impl EuropePmcConnector {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
        })
    }
}

#[derive(Deserialize)]
struct EpmcResponse {
    #[serde(rename = "resultList")]
    result_list: Option<EpmcResultList>,
}

#[derive(Deserialize)]
struct EpmcResultList {
    #[serde(default)]
    result: Vec<EpmcResult>,
}

#[derive(Deserialize)]
struct EpmcResult {
    #[serde(default)]
    id: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    pmid: Option<String>,
    #[serde(default)]
    doi: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(rename = "authorString", default)]
    author_string: Option<String>,
    #[serde(rename = "journalTitle", default)]
    journal_title: Option<String>,
    #[serde(rename = "pubYear", default)]
    pub_year: Option<String>,
    #[serde(rename = "abstractText", default)]
    abstract_text: Option<String>,
    #[serde(rename = "citedByCount", default)]
    cited_by_count: Option<i64>,
}

fn year_filter(q: &str, from: Option<i32>, to: Option<i32>) -> String {
    match (from, to) {
        (Some(f), Some(t)) => format!("({q}) AND (PUB_YEAR:[{f} TO {t}])"),
        (Some(f), None) => format!("({q}) AND (PUB_YEAR:[{f} TO 3000])"),
        (None, Some(t)) => format!("({q}) AND (PUB_YEAR:[0 TO {t}])"),
        (None, None) => q.to_string(),
    }
}

fn split_authors(s: &str) -> Vec<String> {
    s.split(',')
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect()
}

fn map_results(resp: EpmcResponse) -> Vec<LitRecord> {
    resp.result_list
        .map(|rl| rl.result)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| !r.title.trim().is_empty())
        .map(|r| {
            let is_preprint = r.source.eq_ignore_ascii_case("PPR");
            let authors = r.author_string.as_deref().map(split_authors).unwrap_or_default();
            let year = r.pub_year.as_deref().and_then(|y| y.trim().parse::<i32>().ok());
            let url = r
                .doi
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| format!("https://doi.org/{d}"))
                .or_else(|| {
                    (!r.id.is_empty() && !r.source.is_empty())
                        .then(|| format!("https://europepmc.org/article/{}/{}", r.source, r.id))
                });
            LitRecord {
                doi: r.doi.filter(|d| !d.is_empty()),
                pmid: r.pmid.filter(|p| !p.is_empty()),
                title: r.title,
                abstract_text: r.abstract_text.filter(|a| !a.is_empty()),
                authors,
                year,
                venue: r.journal_title.filter(|j| !j.is_empty()),
                url,
                source: "europepmc".into(),
                source_ids: vec![format!("europepmc:{}/{}", r.source, r.id)],
                cited_by_count: r.cited_by_count,
                is_preprint,
                relevance: 0.0,
            }
        })
        .collect()
}

#[async_trait]
impl LitConnector for EuropePmcConnector {
    fn key(&self) -> &'static str {
        "europepmc"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        let q = year_filter(query, opts.year_from, opts.year_to);
        let page_size = opts.limit.clamp(1, 100).to_string();
        let resp = self
            .client
            .get(endpoint(ENDPOINT, "LIT_SEARCH_EUROPEPMC_ENDPOINT"))
            .query(&[
                ("query", q.as_str()),
                ("format", "json"),
                ("resultType", "core"),
                ("pageSize", page_size.as_str()),
            ])
            .header("Accept", "application/json")
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("europepmc request failed: {e}");
                AppError::internal_error("europepmc request failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "europepmc returned HTTP {}",
                resp.status()
            )));
        }
        let parsed: EpmcResponse = read_json_capped(resp, MAX_BODY_BYTES).await?;
        Ok(map_results(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_core_result_and_detects_preprint() {
        let json = serde_json::json!({
            "resultList": { "result": [
                { "id": "PMC1", "source": "MED", "pmid": "111", "doi": "10.1/x",
                  "title": "A study", "authorString": "Smith J, Doe A",
                  "journalTitle": "Nature", "pubYear": "2021",
                  "abstractText": "abstract here", "citedByCount": 42 },
                { "id": "PPR9", "source": "PPR", "title": "A preprint",
                  "authorString": "Roe B", "pubYear": "2023" }
            ]}
        });
        let resp: EpmcResponse = serde_json::from_value(json).unwrap();
        let recs = map_results(resp);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/x"));
        assert_eq!(recs[0].pmid.as_deref(), Some("111"));
        assert_eq!(recs[0].authors, vec!["Smith J", "Doe A"]);
        assert_eq!(recs[0].year, Some(2021));
        assert!(!recs[0].is_preprint);
        assert!(recs[1].is_preprint, "PPR source → preprint");
    }

    #[test]
    fn year_filter_builds_expected_clause() {
        assert_eq!(year_filter("crispr", Some(2020), Some(2022)), "(crispr) AND (PUB_YEAR:[2020 TO 2022])");
        assert_eq!(year_filter("crispr", None, None), "crispr");
    }
}
