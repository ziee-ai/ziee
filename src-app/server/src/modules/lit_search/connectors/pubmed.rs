//! PubMed connector via NCBI E-utilities: `esearch` → `esummary` (JSON
//! metadata) → `efetch` (XML abstracts).
//!
//! `esummary` (JSON) gives title/authors/venue/year + DOI/PMID; a follow-up
//! `efetch` (XML-only) supplies the abstracts so PubMed is **self-sufficient**
//! — its records carry abstracts even when the Europe PMC connector is disabled
//! (rather than silently depending on the EPMC merge for them). An `efetch`
//! failure is non-fatal: records still return, just without native abstracts.

use std::collections::HashMap;

use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Deserialize;
use serde_json::Value;

use super::{
    LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_json_capped,
    read_text_capped,
};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

const ESEARCH: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi";
const ESUMMARY: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi";
const EFETCH: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";

pub struct PubmedConnector {
    client: reqwest::Client,
    mailto: Option<String>,
    api_key: Option<String>,
}

impl PubmedConnector {
    pub fn new(mailto: Option<String>, api_key: Option<String>) -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
            mailto: mailto.filter(|m| !m.trim().is_empty()),
            api_key: api_key.filter(|k| !k.trim().is_empty()),
        })
    }

    /// Common E-utilities params (tool/email identification + optional key).
    fn common_params(&self) -> Vec<(&'static str, String)> {
        let mut p = vec![("db", "pubmed".to_string()), ("tool", "ziee".to_string())];
        if let Some(m) = &self.mailto {
            p.push(("email", m.clone()));
        }
        if let Some(k) = &self.api_key {
            p.push(("api_key", k.clone()));
        }
        p
    }
}

#[derive(Deserialize)]
struct ESearchResponse {
    esearchresult: Option<ESearchResult>,
}

#[derive(Deserialize)]
struct ESearchResult {
    #[serde(default)]
    idlist: Vec<String>,
}

/// Parse the year from a PubMed `pubdate` like "2021 Mar 15" / "2021".
fn parse_year(pubdate: &str) -> Option<i32> {
    pubdate.split_whitespace().next().and_then(|y| y.parse::<i32>().ok())
}

/// Map one esummary entry (a JSON object) to a LitRecord.
fn map_summary(uid: &str, entry: &Value) -> Option<LitRecord> {
    let title = entry.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if title.is_empty() {
        return None;
    }
    let authors = entry
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let venue = entry
        .get("source")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let year = entry
        .get("pubdate")
        .and_then(|v| v.as_str())
        .and_then(parse_year);
    // DOI lives in articleids: [{ idtype, value }, ...].
    let doi = entry
        .get("articleids")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|id| {
                let idtype = id.get("idtype").and_then(|v| v.as_str())?;
                if idtype.eq_ignore_ascii_case("doi") {
                    id.get("value").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
        })
        .filter(|d| !d.is_empty());

    Some(LitRecord {
        doi,
        pmid: Some(uid.to_string()),
        title,
        abstract_text: None, // filled by the efetch pass (map_summary is metadata-only)
        authors,
        year,
        venue,
        url: Some(format!("https://pubmed.ncbi.nlm.nih.gov/{uid}/")),
        source: "pubmed".into(),
        source_ids: vec![format!("pubmed:{uid}")],
        cited_by_count: None,
        is_preprint: false,
        relevance: 0.0,
    })
}

/// Parse an `efetch` PubMed XML set into a `PMID -> abstract text` map.
///
/// Captures the first `<PMID>` of each `<PubmedArticle>` (the MedlineCitation
/// PMID — later reference/comment PMIDs are ignored) and concatenates every
/// `<AbstractText>` segment (structured abstracts prefix the `Label`).
fn parse_abstracts(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut out: HashMap<String, String> = HashMap::new();
    let mut tag: Vec<u8> = Vec::new();
    let mut cur_pmid: Option<String> = None;
    let mut pmid_locked = false; // only the first PMID per article
    let mut in_abstract_text = false;
    let mut cur_abstract = String::new();
    // Per-SECTION buffers, flushed on End(AbstractText) so an empty/whitespace-
    // only section never leaves a dangling "LABEL: " prefix in the output.
    let mut cur_section = String::new();
    let mut cur_label: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"PubmedArticle" => {
                        cur_pmid = None;
                        pmid_locked = false;
                        cur_abstract.clear();
                    }
                    b"AbstractText" => {
                        in_abstract_text = true;
                        cur_section.clear();
                        cur_label = e.attributes().flatten().find_map(|a| {
                            (a.key.as_ref() == b"Label")
                                .then(|| String::from_utf8_lossy(&a.value).trim().to_string())
                                .filter(|l| !l.is_empty())
                        });
                    }
                    _ => {}
                }
                tag = name;
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"AbstractText" => {
                        in_abstract_text = false;
                        // Flush only when the section actually has content.
                        let body = cur_section.trim();
                        if !body.is_empty() {
                            if !cur_abstract.is_empty() {
                                cur_abstract.push('\n');
                            }
                            if let Some(label) = &cur_label {
                                cur_abstract.push_str(label);
                                cur_abstract.push_str(": ");
                            }
                            cur_abstract.push_str(body);
                        }
                        cur_label = None;
                    }
                    b"PubmedArticle" => {
                        if let Some(pmid) = cur_pmid.take() {
                            let abs = cur_abstract.trim();
                            if !abs.is_empty() {
                                out.insert(pmid, abs.to_string());
                            }
                        }
                    }
                    _ => {}
                }
                tag.clear();
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                if tag.as_slice() == b"PMID" && !pmid_locked {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        cur_pmid = Some(trimmed.to_string());
                        pmid_locked = true;
                    }
                } else if in_abstract_text {
                    cur_section.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    out
}

fn map_esummary(body: &Value) -> Vec<LitRecord> {
    let Some(result) = body.get("result") else {
        return vec![];
    };
    let uids: Vec<String> = result
        .get("uids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|u| u.as_str().map(String::from)).collect())
        .unwrap_or_default();
    uids.iter()
        .filter_map(|uid| result.get(uid).and_then(|e| map_summary(uid, e)))
        .collect()
}

#[async_trait]
impl LitConnector for PubmedConnector {
    fn key(&self) -> &'static str {
        "pubmed"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        // 1) esearch → PMIDs.
        let retmax = opts.limit.clamp(1, 100).to_string();
        let mut params = self.common_params();
        params.push(("term", query.to_string()));
        params.push(("retmax", retmax));
        params.push(("retmode", "json".to_string()));
        if opts.year_from.is_some() || opts.year_to.is_some() {
            params.push(("datetype", "pdat".to_string()));
            params.push(("mindate", opts.year_from.unwrap_or(1800).to_string()));
            params.push(("maxdate", opts.year_to.unwrap_or(3000).to_string()));
        }
        let resp = self
            .client
            .get(endpoint(ESEARCH, "LIT_SEARCH_PUBMED_ESEARCH_ENDPOINT"))
            .query(&params)
            .header("Accept", "application/json")
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                // `.without_url()`: the NCBI api_key rides as a URL query param,
                // and reqwest errors embed the full URL — strip it so the key
                // never lands in logs.
                tracing::warn!("pubmed esearch failed: {}", e.without_url());
                AppError::internal_error("pubmed esearch failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "pubmed esearch returned HTTP {}",
                resp.status()
            )));
        }
        let search: ESearchResponse = read_json_capped(resp, MAX_BODY_BYTES).await?;
        let ids = search.esearchresult.map(|r| r.idlist).unwrap_or_default();
        if ids.is_empty() {
            return Ok(vec![]);
        }

        // 2) esummary → metadata.
        let mut params = self.common_params();
        params.push(("id", ids.join(",")));
        params.push(("retmode", "json".to_string()));
        let resp = self
            .client
            .get(endpoint(ESUMMARY, "LIT_SEARCH_PUBMED_ESUMMARY_ENDPOINT"))
            .query(&params)
            .header("Accept", "application/json")
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("pubmed esummary failed: {}", e.without_url());
                AppError::internal_error("pubmed esummary failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "pubmed esummary returned HTTP {}",
                resp.status()
            )));
        }
        let body: Value = read_json_capped(resp, MAX_BODY_BYTES).await?;
        let mut records = map_esummary(&body);

        // 3) efetch → abstracts (XML). Best-effort: a failure leaves records
        //    abstract-less rather than failing the whole connector.
        if !records.is_empty() {
            let abstracts = self.fetch_abstracts(&ids, opts).await.unwrap_or_default();
            if !abstracts.is_empty() {
                for r in &mut records {
                    if let Some(pmid) = &r.pmid
                        && let Some(text) = abstracts.get(pmid)
                    {
                        r.abstract_text = Some(text.clone());
                    }
                }
            }
        }
        Ok(records)
    }
}

impl PubmedConnector {
    /// Fetch abstracts for a set of PMIDs via `efetch` (XML). Returns a
    /// `PMID -> abstract` map; errors map to `Ok(empty)` upstream (best-effort).
    async fn fetch_abstracts(
        &self,
        ids: &[String],
        opts: SearchOpts,
    ) -> Result<HashMap<String, String>, AppError> {
        let mut params = self.common_params();
        params.push(("id", ids.join(",")));
        params.push(("rettype", "abstract".to_string()));
        params.push(("retmode", "xml".to_string()));
        let resp = self
            .client
            .get(endpoint(EFETCH, "LIT_SEARCH_PUBMED_EFETCH_ENDPOINT"))
            .query(&params)
            .header("Accept", "application/xml")
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("pubmed efetch failed: {}", e.without_url());
                AppError::internal_error("pubmed efetch failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "pubmed efetch returned HTTP {}",
                resp.status()
            )));
        }
        let xml = read_text_capped(resp, MAX_BODY_BYTES).await?;
        Ok(parse_abstracts(&xml))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_year_from_pubdate() {
        assert_eq!(parse_year("2021 Mar 15"), Some(2021));
        assert_eq!(parse_year("2019"), Some(2019));
        assert_eq!(parse_year(""), None);
    }

    #[test]
    fn maps_esummary_with_doi_articleid() {
        let body = serde_json::json!({
            "result": {
                "uids": ["111"],
                "111": {
                    "uid": "111",
                    "title": "A PubMed paper",
                    "authors": [{ "name": "Smith J" }, { "name": "Doe A" }],
                    "source": "J Biol",
                    "pubdate": "2020 Jun",
                    "articleids": [
                        { "idtype": "pubmed", "value": "111" },
                        { "idtype": "doi", "value": "10.1/abc" }
                    ]
                }
            }
        });
        let recs = map_esummary(&body);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].pmid.as_deref(), Some("111"));
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/abc"));
        assert_eq!(recs[0].year, Some(2020));
        assert_eq!(recs[0].venue.as_deref(), Some("J Biol"));
        assert_eq!(recs[0].authors.len(), 2);
        assert!(recs[0].abstract_text.is_none());
    }

    #[test]
    fn parses_efetch_abstracts_keyed_by_citation_pmid() {
        // Two articles; #222 has a structured abstract, #333 a plain one.
        // A trailing CommentsCorrections PMID must NOT clobber the citation PMID.
        let xml = r#"
        <PubmedArticleSet>
          <PubmedArticle>
            <MedlineCitation>
              <PMID Version="1">222</PMID>
              <Article>
                <Abstract>
                  <AbstractText Label="BACKGROUND">Cells divide.</AbstractText>
                  <AbstractText Label="RESULTS">They divided faster.</AbstractText>
                </Abstract>
              </Article>
            </MedlineCitation>
            <PubmedData>
              <CommentsCorrectionsList>
                <CommentsCorrections><PMID>999</PMID></CommentsCorrections>
              </CommentsCorrectionsList>
            </PubmedData>
          </PubmedArticle>
          <PubmedArticle>
            <MedlineCitation>
              <PMID Version="1">333</PMID>
              <Article>
                <Abstract><AbstractText>A plain abstract.</AbstractText></Abstract>
              </Article>
            </MedlineCitation>
          </PubmedArticle>
        </PubmedArticleSet>"#;
        let map = parse_abstracts(xml);
        assert_eq!(map.len(), 2);
        let a = map.get("222").expect("222 present");
        assert!(a.contains("BACKGROUND: Cells divide."), "got: {a}");
        assert!(a.contains("RESULTS: They divided faster."), "got: {a}");
        assert_eq!(map.get("333").map(|s| s.as_str()), Some("A plain abstract."));
        assert!(!map.contains_key("999"), "reference PMID must not be keyed");
    }

    #[test]
    fn efetch_skips_empty_labeled_sections_no_dangling_prefix() {
        // An empty (self-closed/whitespace) labeled section must NOT leave a
        // dangling "METHODS: " in the output.
        let xml = r#"
        <PubmedArticleSet>
          <PubmedArticle>
            <MedlineCitation>
              <PMID Version="1">555</PMID>
              <Article>
                <Abstract>
                  <AbstractText Label="BACKGROUND">Real background.</AbstractText>
                  <AbstractText Label="METHODS"></AbstractText>
                  <AbstractText Label="RESULTS">   </AbstractText>
                  <AbstractText Label="CONCLUSIONS">Real conclusion.</AbstractText>
                </Abstract>
              </Article>
            </MedlineCitation>
          </PubmedArticle>
        </PubmedArticleSet>"#;
        let map = parse_abstracts(xml);
        let a = map.get("555").expect("555 present");
        assert!(a.contains("BACKGROUND: Real background."), "got: {a}");
        assert!(a.contains("CONCLUSIONS: Real conclusion."), "got: {a}");
        assert!(!a.contains("METHODS:"), "empty section must not emit a label: {a}");
        assert!(!a.contains("RESULTS:"), "whitespace-only section must not emit a label: {a}");
    }
}
