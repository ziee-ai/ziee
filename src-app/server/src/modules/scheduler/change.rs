//! Pure change-detection engine (DEC-20) — no I/O, unit-testable.
//!
//! Reduces a firing's result text to a stable `fingerprint` (for the
//! coarse "did anything change" decision) plus an `item-set` of identifiable
//! scholarly IDs (DOI/PMID/arXiv/URL) for an EXACT delta ("N new since last
//! run"). A `notify_on='on_change'` task suppresses its notification when the
//! fingerprint is unchanged; otherwise the delta leads the notification body.
//!
//! Item extraction reuses `lit_search::dedup::normalize_doi` so a DOI diff is
//! exact rather than an LLM guess.

use std::collections::BTreeSet;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::modules::lit_search::dedup::normalize_doi;

/// The persisted reduction of a result (stored in
/// `scheduled_tasks.last_result_signature_json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub fingerprint: String,
    /// Sorted, de-duplicated identifiable items (may be empty for free text).
    pub items: Vec<String>,
}

/// The result of diffing a fresh signature against the previous one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeOutcome {
    /// Whether the result meaningfully changed (drives on_change suppression).
    pub changed: bool,
    /// Items present now but not last run (the "N new" delta). Empty when the
    /// result carries no identifiable items.
    pub new_items: Vec<String>,
}

static DOI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b10\.\d{4,9}/[-._;()/:a-z0-9]+").unwrap());
static ARXIV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\barxiv:\s*(\d{4}\.\d{4,5})(v\d+)?").unwrap());
static PMID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bpmid:?\s*(\d{5,9})\b").unwrap());
static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://[^\s)>\]]+").unwrap());

/// Normalize text for the fingerprint: lowercase + collapse all whitespace runs
/// to a single space + trim. Makes the fingerprint stable against benign
/// reflow / casing / trailing-whitespace volatility.
fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Extract identifiable items (DOI/arXiv/PMID/URL), normalized + de-duplicated.
fn extract_items(text: &str) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();

    for m in DOI_RE.find_iter(text) {
        if let Some(doi) = normalize_doi(m.as_str()) {
            set.insert(format!("doi:{doi}"));
        }
    }
    for c in ARXIV_RE.captures_iter(text) {
        set.insert(format!("arxiv:{}", &c[1]));
    }
    for c in PMID_RE.captures_iter(text) {
        set.insert(format!("pmid:{}", &c[1]));
    }
    for m in URL_RE.find_iter(text) {
        // Skip URLs that are just a DOI/arXiv landing page (already captured).
        let u = m.as_str().trim_end_matches(['.', ',']).to_lowercase();
        if DOI_RE.is_match(&u) {
            continue;
        }
        set.insert(format!("url:{u}"));
    }

    set.into_iter().collect()
}

/// Reduce a result to its signature.
pub fn compute_signature(result_text: &str) -> Signature {
    let normalized = normalize_text(result_text);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let fingerprint = hex::encode(hasher.finalize());
    Signature {
        fingerprint,
        items: extract_items(result_text),
    }
}

/// Diff a fresh signature against the previous one (None = first run).
///
/// `changed` is true on the first run and whenever the fingerprint differs.
/// `new_items` is the set difference (present now, absent before) — the exact
/// delta when the result carries identifiable items.
pub fn diff(prev: Option<&Signature>, curr: &Signature) -> ChangeOutcome {
    match prev {
        None => ChangeOutcome {
            changed: true,
            new_items: curr.items.clone(),
        },
        Some(prev) => {
            let prev_set: BTreeSet<&String> = prev.items.iter().collect();
            let new_items: Vec<String> = curr
                .items
                .iter()
                .filter(|i| !prev_set.contains(*i))
                .cloned()
                .collect();
            ChangeOutcome {
                changed: curr.fingerprint != prev.fingerprint,
                new_items,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-42: fingerprint stable across benign volatility, differs on content.
    #[test]
    fn fingerprint_is_reflow_stable() {
        let a = compute_signature("Found 3 papers on CRISPR.\n\nSee 10.1000/xyz.");
        let b = compute_signature("  found   3 PAPERS on crispr.   see 10.1000/xyz. ");
        assert_eq!(a.fingerprint, b.fingerprint, "reflow/case should not change fp");

        let c = compute_signature("Found 4 papers on CRISPR. See 10.1000/xyz.");
        assert_ne!(a.fingerprint, c.fingerprint, "real content change flips fp");
    }

    // TEST-42: item extraction pulls normalized DOIs/arXiv/PMIDs.
    #[test]
    fn extracts_identifiable_items() {
        let sig = compute_signature(
            "New: 10.1234/abc.def and arXiv:2501.01234v2 and PMID: 40123456",
        );
        assert!(sig.items.contains(&"doi:10.1234/abc.def".to_string()));
        assert!(sig.items.contains(&"arxiv:2501.01234".to_string()));
        assert!(sig.items.contains(&"pmid:40123456".to_string()));
    }

    // TEST-42: set-diff yields exactly the added items; unchanged → not changed.
    #[test]
    fn diff_reports_only_new_items() {
        let prev = compute_signature("Papers: 10.1000/a, 10.2000/b");
        let curr = compute_signature("Papers: 10.1000/a, 10.2000/b, 10.3000/c");
        let out = diff(Some(&prev), &curr);
        assert!(out.changed);
        assert_eq!(out.new_items, vec!["doi:10.3000/c".to_string()]);

        // Identical result → not changed, no new items.
        let same = diff(Some(&prev), &compute_signature("Papers: 10.1000/a, 10.2000/b"));
        assert!(!same.changed);
        assert!(same.new_items.is_empty());

        // First run → changed, all items new.
        let first = diff(None, &curr);
        assert!(first.changed);
        assert_eq!(first.new_items.len(), 3);
    }
}
