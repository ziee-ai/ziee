//! Crossref connector — canonical DOI metadata, JSON REST. Abstracts are often
//! absent (JATS when present). Optional `mailto` (polite pool) + optional
//! Crossref Plus bearer token.

use async_trait::async_trait;
use serde::Deserialize;

use super::{LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_json_capped};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

const ENDPOINT: &str = "https://api.crossref.org/works";

pub struct CrossrefConnector {
    client: reqwest::Client,
    mailto: Option<String>,
    plus_token: Option<String>,
}

impl CrossrefConnector {
    pub fn new(mailto: Option<String>, plus_token: Option<String>) -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
            mailto: mailto.filter(|m| !m.trim().is_empty()),
            plus_token: plus_token.filter(|t| !t.trim().is_empty()),
        })
    }
}

#[derive(Deserialize)]
struct CrossrefResponse {
    message: Option<CrossrefMessage>,
}

#[derive(Deserialize)]
struct CrossrefMessage {
    #[serde(default)]
    items: Vec<CrossrefItem>,
}

#[derive(Deserialize)]
struct CrossrefItem {
    #[serde(rename = "DOI", default)]
    doi: Option<String>,
    #[serde(default)]
    title: Vec<String>,
    #[serde(default)]
    author: Vec<CrossrefAuthor>,
    #[serde(rename = "container-title", default)]
    container_title: Vec<String>,
    #[serde(default)]
    issued: Option<CrossrefDate>,
    #[serde(default)]
    r#abstract: Option<String>,
    #[serde(rename = "is-referenced-by-count", default)]
    referenced_by: Option<i64>,
    #[serde(rename = "type", default)]
    work_type: Option<String>,
    #[serde(rename = "URL", default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct CrossrefAuthor {
    #[serde(default)]
    given: Option<String>,
    #[serde(default)]
    family: Option<String>,
}

#[derive(Deserialize)]
struct CrossrefDate {
    #[serde(rename = "date-parts", default)]
    date_parts: Vec<Vec<i64>>,
}

fn author_name(a: &CrossrefAuthor) -> Option<String> {
    match (a.given.as_deref(), a.family.as_deref()) {
        (Some(g), Some(f)) => Some(format!("{f} {g}")),
        (None, Some(f)) => Some(f.to_string()),
        (Some(g), None) => Some(g.to_string()),
        (None, None) => None,
    }
}

/// Crudely strip JATS/XML tags from a Crossref abstract (best-effort).
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn map_results(resp: CrossrefResponse) -> Vec<LitRecord> {
    resp.message
        .map(|m| m.items)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|it| {
            let title = it.title.into_iter().find(|t| !t.trim().is_empty())?;
            let year = it
                .issued
                .as_ref()
                .and_then(|d| d.date_parts.first())
                .and_then(|p| p.first())
                .map(|y| *y as i32);
            let authors = it.author.iter().filter_map(author_name).collect();
            let is_preprint = it
                .work_type
                .as_deref()
                .map(|t| t == "posted-content")
                .unwrap_or(false);
            let doi = it.doi.filter(|d| !d.is_empty());
            let url = it
                .url
                .filter(|u| !u.is_empty())
                .or_else(|| doi.as_deref().map(|d| format!("https://doi.org/{d}")));
            Some(LitRecord {
                doi: doi.clone(),
                pmid: None,
                title,
                abstract_text: it.r#abstract.as_deref().map(strip_tags).filter(|a| !a.is_empty()),
                authors,
                year,
                venue: it.container_title.into_iter().find(|c| !c.trim().is_empty()),
                url,
                source: "crossref".into(),
                source_ids: vec![format!("crossref:{}", doi.unwrap_or_default())],
                cited_by_count: it.referenced_by,
                is_preprint,
                relevance: 0.0,
            })
        })
        .collect()
}

#[async_trait]
impl LitConnector for CrossrefConnector {
    fn key(&self) -> &'static str {
        "crossref"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        let rows = opts.limit.clamp(1, 100).to_string();
        let mut params: Vec<(&str, String)> = vec![
            ("query", query.to_string()),
            ("rows", rows),
            (
                "select",
                "DOI,title,author,container-title,issued,abstract,is-referenced-by-count,type,URL"
                    .to_string(),
            ),
        ];
        if let Some(m) = &self.mailto {
            params.push(("mailto", m.clone()));
        }
        let mut filters: Vec<String> = Vec::new();
        if let Some(f) = opts.year_from {
            filters.push(format!("from-pub-date:{f}-01-01"));
        }
        if let Some(t) = opts.year_to {
            filters.push(format!("until-pub-date:{t}-12-31"));
        }
        if !filters.is_empty() {
            params.push(("filter", filters.join(",")));
        }

        let ua = match &self.mailto {
            Some(m) => format!("ziee/{} (mailto:{m})", env!("CARGO_PKG_VERSION")),
            None => format!("ziee/{}", env!("CARGO_PKG_VERSION")),
        };
        let mut req = self
            .client
            .get(endpoint(ENDPOINT, "LIT_SEARCH_CROSSREF_ENDPOINT"))
            .query(&params)
            .header("User-Agent", ua)
            .header("Accept", "application/json")
            .timeout(opts.timeout);
        if let Some(tok) = &self.plus_token {
            req = req.header("Crossref-Plus-API-Token", format!("Bearer {tok}"));
        }
        let resp = req.send().await.map_err(|e| {
            tracing::warn!("crossref request failed: {e}");
            AppError::internal_error("crossref request failed")
        })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "crossref returned HTTP {}",
                resp.status()
            )));
        }
        let parsed: CrossrefResponse = read_json_capped(resp, MAX_BODY_BYTES).await?;
        Ok(map_results(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_item_with_nested_arrays() {
        let json = serde_json::json!({
            "message": { "items": [{
                "DOI": "10.1/x",
                "title": ["A title"],
                "author": [{ "given": "Jane", "family": "Smith" }],
                "container-title": ["Nature"],
                "issued": { "date-parts": [[2020, 5, 1]] },
                "abstract": "<jats:p>Some <b>abstract</b></jats:p>",
                "is-referenced-by-count": 7,
                "type": "journal-article"
            }]}
        });
        let resp: CrossrefResponse = serde_json::from_value(json).unwrap();
        let recs = map_results(resp);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/x"));
        assert_eq!(recs[0].title, "A title");
        assert_eq!(recs[0].authors, vec!["Smith Jane"]);
        assert_eq!(recs[0].year, Some(2020));
        assert_eq!(recs[0].venue.as_deref(), Some("Nature"));
        assert_eq!(recs[0].abstract_text.as_deref(), Some("Some abstract"));
        assert!(!recs[0].is_preprint);
    }

    #[test]
    fn posted_content_is_preprint() {
        let json = serde_json::json!({
            "message": { "items": [{ "DOI": "10.1/p", "title": ["P"], "type": "posted-content" }]}
        });
        let resp: CrossrefResponse = serde_json::from_value(json).unwrap();
        assert!(map_results(resp)[0].is_preprint);
    }
}
