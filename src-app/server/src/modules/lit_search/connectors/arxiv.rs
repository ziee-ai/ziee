//! arXiv connector — CS / methods / quant-bio preprints. The arXiv API is Atom
//! XML only (no JSON), so we parse it with quick-xml. No key. DOIs are present
//! only when the author supplied one (`<arxiv:doi>`), so most arXiv records dedup
//! by (title, year). Every record is a preprint.

use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use super::{LitConnector, MAX_BODY_BYTES, SearchOpts, build_client, endpoint, read_text_capped};
use crate::common::AppError;
use crate::modules::lit_search::models::LitRecord;

// HTTPS (arXiv's export endpoint supports TLS) so query terms aren't sent in
// cleartext — consistent with the module's connected-only/IP-sensitivity posture.
const ENDPOINT: &str = "https://export.arxiv.org/api/query";

pub struct ArxivConnector {
    client: reqwest::Client,
}

impl ArxivConnector {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: build_client()?,
        })
    }
}

#[derive(Default)]
struct Entry {
    title: String,
    summary: String,
    published: String,
    id: String,
    doi: Option<String>,
    authors: Vec<String>,
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Entry {
    fn into_record(self) -> Option<LitRecord> {
        let title = collapse_ws(&self.title);
        if title.is_empty() {
            return None;
        }
        // `<id>` is a URL like http://arxiv.org/abs/2201.00001v1 → the part after /abs/.
        let arxiv_id = self.id.rsplit("/abs/").next().unwrap_or("").trim().to_string();
        let year = self.published.get(0..4).and_then(|y| y.parse::<i32>().ok());
        let abstract_text = {
            let a = collapse_ws(&self.summary);
            (!a.is_empty()).then_some(a)
        };
        let id_for_source = if arxiv_id.is_empty() {
            self.id.clone()
        } else {
            arxiv_id.clone()
        };
        let url = if !arxiv_id.is_empty() {
            Some(format!("https://arxiv.org/abs/{arxiv_id}"))
        } else {
            (!self.id.is_empty()).then(|| self.id.clone())
        };
        Some(LitRecord {
            doi: self.doi.filter(|d| !d.trim().is_empty()),
            pmid: None,
            title,
            abstract_text,
            authors: self.authors,
            year,
            venue: Some("arXiv".to_string()),
            url,
            source: "arxiv".into(),
            source_ids: vec![format!("arxiv:{id_for_source}")],
            cited_by_count: None,
            is_preprint: true,
            relevance: 0.0,
        })
    }
}

fn parse_atom(xml: &str) -> Vec<LitRecord> {
    let mut reader = Reader::from_str(xml);
    let mut records = Vec::new();
    let mut in_entry = false;
    let mut in_author = false;
    // title/summary ACCUMULATE text and may contain inline children — gate them
    // on booleans (like pubmed's `in_abstract_text`) so a nested child's End
    // doesn't truncate the field. `tag` drives only the one-shot leaf fields.
    let mut in_title = false;
    let mut in_summary = false;
    let mut cur: Option<Entry> = None;
    let mut tag: Vec<u8> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                match name.as_slice() {
                    b"entry" => {
                        in_entry = true;
                        cur = Some(Entry::default());
                    }
                    b"author" if in_entry => in_author = true,
                    b"title" if in_entry => in_title = true,
                    b"summary" if in_entry => in_summary = true,
                    _ => {}
                }
                tag = name;
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"entry" => {
                        if let Some(entry) = cur.take()
                            && let Some(r) = entry.into_record()
                        {
                            records.push(r);
                        }
                        in_entry = false;
                    }
                    b"author" => in_author = false,
                    b"title" => in_title = false,
                    b"summary" => in_summary = false,
                    _ => {}
                }
                tag.clear();
            }
            Ok(Event::Text(t)) => {
                if in_entry
                    && let Some(entry) = cur.as_mut()
                {
                    let text = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                    // Accumulating fields: append ALL text (incl. nested-child
                    // text) verbatim until the field's End.
                    if in_title {
                        entry.title.push_str(&text);
                    } else if in_summary {
                        entry.summary.push_str(&text);
                    } else if !text.trim().is_empty() {
                        match tag.as_slice() {
                            b"published" if entry.published.is_empty() => {
                                entry.published.push_str(text.trim())
                            }
                            b"id" if entry.id.is_empty() => entry.id.push_str(text.trim()),
                            b"arxiv:doi" => entry.doi = Some(text.trim().to_string()),
                            b"name" if in_author => entry.authors.push(text.trim().to_string()),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    records
}

#[async_trait]
impl LitConnector for ArxivConnector {
    fn key(&self) -> &'static str {
        "arxiv"
    }

    async fn search(&self, query: &str, opts: SearchOpts) -> Result<Vec<LitRecord>, AppError> {
        // arXiv's API has no clean publication-year filter on search_query, so we
        // fetch unfiltered then POST-FILTER on the parsed year below — otherwise
        // arXiv would be the one source silently returning out-of-range preprints
        // when the caller requests a year range (the other five honor it upstream).
        let max = opts.limit.clamp(1, 100).to_string();
        let resp = self
            .client
            .get(endpoint(ENDPOINT, "LIT_SEARCH_ARXIV_ENDPOINT"))
            .query(&[
                ("search_query", format!("all:{query}").as_str()),
                ("start", "0"),
                ("max_results", max.as_str()),
            ])
            .timeout(opts.timeout)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("arxiv request failed: {e}");
                AppError::internal_error("arxiv request failed")
            })?;
        if !resp.status().is_success() {
            return Err(AppError::internal_error(format!(
                "arxiv returned HTTP {}",
                resp.status()
            )));
        }
        let xml = read_text_capped(resp, MAX_BODY_BYTES).await?;
        let mut records = parse_atom(&xml);
        // Honor the requested year range (parity with the other connectors).
        // A record with no parsed year is kept (can't prove it's out of range).
        if opts.year_from.is_some() || opts.year_to.is_some() {
            records.retain(|r| match r.year {
                Some(y) => {
                    opts.year_from.map_or(true, |lo| y >= lo)
                        && opts.year_to.map_or(true, |hi| y <= hi)
                }
                None => true,
            });
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
      <id>http://arxiv.org/api/feed</id>
      <title>ArXiv Query</title>
      <entry>
        <id>http://arxiv.org/abs/2201.00001v1</id>
        <published>2022-01-03T00:00:00Z</published>
        <title>A Quant-Bio Method</title>
        <summary>We present a method for X.</summary>
        <author><name>Jane Smith</name></author>
        <author><name>Bob Roe</name></author>
        <arxiv:doi>10.1/arxivdoi</arxiv:doi>
      </entry>
      <entry>
        <id>http://arxiv.org/abs/2203.04567v2</id>
        <published>2022-03-09T00:00:00Z</published>
        <title>No DOI Here</title>
        <summary>Another abstract.</summary>
        <author><name>A. Author</name></author>
      </entry>
    </feed>"#;

    #[test]
    fn parses_entries_authors_doi_and_preprint_flag() {
        let recs = parse_atom(FEED);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].title, "A Quant-Bio Method");
        assert_eq!(recs[0].year, Some(2022));
        assert_eq!(recs[0].authors, vec!["Jane Smith", "Bob Roe"]);
        assert_eq!(recs[0].doi.as_deref(), Some("10.1/arxivdoi"));
        assert_eq!(recs[0].source_ids, vec!["arxiv:2201.00001v1"]);
        assert!(recs[0].is_preprint);
        // Second entry has no DOI.
        assert!(recs[1].doi.is_none());
        assert_eq!(recs[1].title, "No DOI Here");
    }

    #[test]
    fn title_and_summary_survive_inline_child_elements() {
        // A nested child inside <title>/<summary> must NOT truncate the field
        // (regression: shared `tag` cleared on every End dropped post-child text).
        let feed = r#"<?xml version="1.0"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <id>http://arxiv.org/abs/2204.00002v1</id>
            <published>2022-04-01T00:00:00Z</published>
            <title>Before <sub>nested</sub> after</title>
            <summary>Summary start <i>mid</i> summary end.</summary>
            <author><name>X. Y</name></author>
          </entry>
        </feed>"#;
        let recs = parse_atom(feed);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].title, "Before nested after");
        assert_eq!(recs[0].abstract_text.as_deref(), Some("Summary start mid summary end."));
    }
}
