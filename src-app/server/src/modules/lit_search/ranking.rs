//! Pure heuristic relevance ranking (no I/O).
//!
//! Deterministic + transparent — NOT a learned model. Score = query-term coverage
//! in the title (high weight) + abstract (low weight, gracefully zero when the
//! abstract is absent so Crossref-only records aren't unfairly buried) + a mild
//! recency boost + a log-scaled `cited_by_count` tie-breaker, normalized to 0..1.
//!
//! The function shape is "active-learning ready": a future
//! `rank_with_labels(records, decisions)` can re-prioritize from include/exclude
//! decisions without changing callers.

use super::models::LitRecord;

const W_TITLE: f32 = 0.60;
const W_ABSTRACT: f32 = 0.20;
const W_RECENCY: f32 = 0.12;
const W_CITES: f32 = 0.08;

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(String::from)
        .collect()
}

/// Fraction of distinct query terms that appear in `text` (0..1).
fn coverage(query_terms: &[String], text: &str) -> f32 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let hay = text.to_lowercase();
    let hits = query_terms.iter().filter(|t| hay.contains(t.as_str())).count();
    hits as f32 / query_terms.len() as f32
}

/// Score a single record in 0..1 against the query terms.
fn score(record: &LitRecord, query_terms: &[String], current_year: i32) -> f32 {
    let title = coverage(query_terms, &record.title);
    let abstract_cov = record
        .abstract_text
        .as_deref()
        .map(|a| coverage(query_terms, a))
        .unwrap_or(0.0);
    // Recency: linear decay over ~15 years; unknown year → neutral 0.5.
    let recency = match record.year {
        Some(y) => {
            let age = (current_year - y).max(0) as f32;
            (1.0 - (age / 15.0)).clamp(0.0, 1.0)
        }
        None => 0.5,
    };
    // Citations: log-scaled tie-breaker, saturating around ~1000 cites.
    let cites = record
        .cited_by_count
        .filter(|&c| c > 0)
        .map(|c| ((c as f32).ln() / 1000f32.ln()).clamp(0.0, 1.0))
        .unwrap_or(0.0);

    (W_TITLE * title + W_ABSTRACT * abstract_cov + W_RECENCY * recency + W_CITES * cites)
        .clamp(0.0, 1.0)
}

/// Rank records in place: fill `relevance` and sort descending (stable; ties keep
/// input order). `current_year` is injected (the runtime stamps it) so this stays
/// pure and unit-testable.
pub fn rank(records: &mut [LitRecord], query: &str, current_year: i32) {
    let terms = tokenize(query);
    for r in records.iter_mut() {
        r.relevance = score(r, &terms, current_year);
    }
    records.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(title: &str, abs: Option<&str>, year: Option<i32>, cites: Option<i64>) -> LitRecord {
        LitRecord {
            doi: None,
            pmid: None,
            title: title.to_string(),
            abstract_text: abs.map(String::from),
            authors: vec![],
            year,
            venue: None,
            url: None,
            source: "t".into(),
            source_ids: vec![],
            cited_by_count: cites,
            is_preprint: false,
            relevance: 0.0,
        }
    }

    #[test]
    fn title_match_outranks_abstract_match() {
        let mut v = vec![
            rec("unrelated heading", Some("crispr base editing here"), Some(2022), None),
            rec("crispr base editing", None, Some(2022), None),
        ];
        rank(&mut v, "crispr base editing", 2024);
        assert_eq!(v[0].title, "crispr base editing", "title hit should rank first");
        assert!(v[0].relevance > v[1].relevance);
    }

    #[test]
    fn missing_abstract_does_not_bury_a_title_match() {
        // A record with NO abstract but a full title match must still score well.
        let mut v = vec![rec("crispr base editing off target", None, Some(2023), None)];
        rank(&mut v, "crispr base editing", 2024);
        assert!(v[0].relevance >= W_TITLE * 0.99, "full title coverage should dominate");
    }

    #[test]
    fn deterministic_and_sorted_desc() {
        let mut v = vec![
            rec("a", None, Some(2000), None),
            rec("crispr", None, Some(2024), None),
        ];
        rank(&mut v, "crispr", 2024);
        assert!(v[0].relevance >= v[1].relevance);
    }
}
