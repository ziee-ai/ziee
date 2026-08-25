//! Pure dedup of literature records (no I/O).
//!
//! DOI-first: normalize DOIs and group by them; DOI-less records key on a
//! normalized `(title, year)` fingerprint (covers DOI-less arXiv preprints).
//! Merge keeps the richest field set and accumulates `source_ids` so the merged
//! record's distinct-source count drives the capture-recapture overlap signal.
//! Fuzzy title matching is out of scope (human screening absorbs residual dupes).

use std::collections::BTreeMap;

use super::models::LitRecord;

/// Normalize a DOI for grouping: strip scheme / `doi:` prefix, lowercase, trim.
/// DOIs are case-insensitive. Returns `None` for empty/clearly-non-DOI input.
pub fn normalize_doi(raw: &str) -> Option<String> {
    let s = raw.trim().to_lowercase();
    let s = s
        .strip_prefix("https://doi.org/")
        .or_else(|| s.strip_prefix("http://doi.org/"))
        .or_else(|| s.strip_prefix("doi.org/"))
        .or_else(|| s.strip_prefix("doi:"))
        .unwrap_or(&s)
        .trim();
    if s.is_empty() || !s.starts_with("10.") {
        return None;
    }
    Some(s.to_string())
}

/// Normalized `(title, year)` fingerprint for DOI-less records: lowercase, keep
/// only alphanumerics + single spaces, append the year.
fn title_year_key(record: &LitRecord) -> String {
    let mut t = String::with_capacity(record.title.len());
    let mut prev_space = false;
    for c in record.title.chars() {
        if c.is_alphanumeric() {
            t.extend(c.to_lowercase());
            prev_space = false;
        } else if !prev_space {
            t.push(' ');
            prev_space = true;
        }
    }
    let t = t.trim();
    format!("{}|{}", t, record.year.map(|y| y.to_string()).unwrap_or_default())
}

/// The grouping key for a record: its normalized DOI if present, else the
/// `(title, year)` fingerprint.
fn group_key(record: &LitRecord) -> String {
    record
        .doi
        .as_deref()
        .and_then(normalize_doi)
        .map(|d| format!("doi:{d}"))
        .unwrap_or_else(|| format!("ty:{}", title_year_key(record)))
}

/// Merge `b` into `a`, keeping the richer fields and accumulating provenance.
fn merge_into(a: &mut LitRecord, b: LitRecord) {
    // Longest non-empty abstract wins.
    let a_len = a.abstract_text.as_deref().map(str::len).unwrap_or(0);
    let b_len = b.abstract_text.as_deref().map(str::len).unwrap_or(0);
    if b_len > a_len {
        a.abstract_text = b.abstract_text;
    }
    // Prefer a longer/non-empty title.
    if b.title.len() > a.title.len() {
        a.title = b.title;
    }
    a.doi = a.doi.take().or(b.doi);
    a.pmid = a.pmid.take().or(b.pmid);
    a.year = a.year.or(b.year);
    a.venue = a.venue.take().or(b.venue);
    a.url = a.url.take().or(b.url);
    a.is_preprint = a.is_preprint && b.is_preprint; // a published copy wins over preprint
    a.cited_by_count = match (a.cited_by_count, b.cited_by_count) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, y) => x.or(y),
    };
    // Union authors (dedup, preserve order; prefer the longer list as the base).
    if b.authors.len() > a.authors.len() {
        let mut merged = b.authors;
        for au in std::mem::take(&mut a.authors) {
            if !merged.iter().any(|x| x.eq_ignore_ascii_case(&au)) {
                merged.push(au);
            }
        }
        a.authors = merged;
    } else {
        for au in b.authors {
            if !a.authors.iter().any(|x| x.eq_ignore_ascii_case(&au)) {
                a.authors.push(au);
            }
        }
    }
    // Accumulate source ids (dedup).
    for sid in b.source_ids {
        if !a.source_ids.contains(&sid) {
            a.source_ids.push(sid);
        }
    }
}

/// Dedup + merge a flat list of records (already collected across sources).
/// Returns the merged records in first-seen order; normalize each record's DOI
/// to the canonical form on the way out.
pub fn merge_by_doi(records: Vec<LitRecord>) -> Vec<LitRecord> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: BTreeMap<String, LitRecord> = BTreeMap::new();
    for mut rec in records {
        // Canonicalize the DOI in place so output records carry the normalized id.
        if let Some(d) = rec.doi.as_deref().and_then(normalize_doi) {
            rec.doi = Some(d);
        }
        let key = group_key(&rec);
        match groups.get_mut(&key) {
            Some(existing) => merge_into(existing, rec),
            None => {
                order.push(key.clone());
                groups.insert(key, rec);
            }
        }
    }
    order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .collect()
}

/// Distinct-source count for a merged record (how many connectors saw it) — the
/// input to the capture-recapture overlap signal. Derived from `source_ids`
/// prefixes (the part before the first `:`).
pub fn distinct_sources(record: &LitRecord) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for sid in &record.source_ids {
        let src = sid.split(':').next().unwrap_or(sid);
        if !seen.contains(&src) {
            seen.push(src);
        }
    }
    seen.len().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(doi: Option<&str>, title: &str, year: Option<i32>, source: &str, abs: Option<&str>) -> LitRecord {
        LitRecord {
            doi: doi.map(String::from),
            pmid: None,
            title: title.to_string(),
            abstract_text: abs.map(String::from),
            authors: vec![],
            year,
            venue: None,
            url: None,
            source: source.to_string(),
            source_ids: vec![format!("{source}:x")],
            cited_by_count: None,
            is_preprint: false,
            relevance: 0.0,
        }
    }

    #[test]
    fn doi_normalization_collapses_variants() {
        assert_eq!(normalize_doi("https://doi.org/10.1/AbC").as_deref(), Some("10.1/abc"));
        assert_eq!(normalize_doi("doi:10.1/abc").as_deref(), Some("10.1/abc"));
        assert_eq!(normalize_doi("10.1/ABC").as_deref(), Some("10.1/abc"));
        assert_eq!(normalize_doi("not-a-doi"), None);
        assert_eq!(normalize_doi(""), None);
    }

    #[test]
    fn merges_same_doi_keeps_longest_abstract_and_accumulates_sources() {
        let a = rec(Some("10.1/x"), "Title", Some(2020), "europepmc", Some("short"));
        let b = rec(Some("HTTPS://doi.org/10.1/X"), "Title", Some(2020), "crossref", Some("a much longer abstract"));
        let merged = merge_by_doi(vec![a, b]);
        assert_eq!(merged.len(), 1, "same DOI should merge");
        assert_eq!(merged[0].abstract_text.as_deref(), Some("a much longer abstract"));
        assert_eq!(distinct_sources(&merged[0]), 2);
        assert_eq!(merged[0].doi.as_deref(), Some("10.1/x"));
    }

    #[test]
    fn doi_less_records_merge_by_title_year() {
        let a = rec(None, "CRISPR base editing!", Some(2021), "arxiv", None);
        let b = rec(None, "crispr   base  editing", Some(2021), "europepmc", Some("abs"));
        let merged = merge_by_doi(vec![a, b]);
        assert_eq!(merged.len(), 1, "same normalized (title,year) should merge");
        assert_eq!(distinct_sources(&merged[0]), 2);
    }

    #[test]
    fn distinct_records_do_not_merge() {
        let a = rec(Some("10.1/a"), "A", Some(2020), "europepmc", None);
        let b = rec(Some("10.1/b"), "B", Some(2020), "crossref", None);
        assert_eq!(merge_by_doi(vec![a, b]).len(), 2);
    }
}
