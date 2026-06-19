//! Pure completeness/saturation estimate (no I/O).
//!
//! Reports a CONSERVATIVE, qualitative saturation signal — NEVER a recall
//! percentage. Two methodologically-defensible signals (the deep-research pass):
//!   * **capture-recapture overlap** — the fraction of top records found by ≥2
//!     independent sources. High agreement is weak evidence the indexes broadly
//!     converge; low agreement means sources surface disjoint sets (search likely
//!     NOT saturated).
//!   * **source breadth** — how many sources actually returned results.
//! Bucketed into low/moderate/high with an explicit adjunct caveat.

use std::collections::BTreeMap;

use super::dedup::distinct_sources;
use super::models::{CompletenessEstimate, LitRecord};

const CAVEAT: &str = "Heuristic saturation signal based on cross-source agreement — NOT a measured recall rate. Relevant work may be missing; this is an adjunct to, not a replacement for, systematic searching. Broaden the query or add sources if coverage matters.";

/// Estimate saturation over the deduped, ranked records + per-source identified
/// counts. `records` should already be ranked (we look at the top slice).
pub fn estimate(records: &[LitRecord], identified: &BTreeMap<String, usize>) -> CompletenessEstimate {
    let sources_with_hits = identified.values().filter(|&&c| c > 0).count();

    // Capture-recapture overlap over the top records: fraction found by ≥2 sources.
    let top_n = records.len().min(20);
    let overlap_frac = if top_n == 0 {
        0.0
    } else {
        let multi = records[..top_n]
            .iter()
            .filter(|r| distinct_sources(r) >= 2)
            .count();
        multi as f32 / top_n as f32
    };

    // Bucket: need both breadth (≥2 sources returned anything) and meaningful
    // cross-source agreement to call it anything above "low".
    let estimate = if top_n == 0 || sources_with_hits < 2 {
        "low"
    } else if overlap_frac >= 0.5 {
        "high"
    } else if overlap_frac >= 0.2 {
        "moderate"
    } else {
        "low"
    };

    let method = format!(
        "cross-source overlap: {:.0}% of top {} records found by ≥2 of {} responding sources",
        overlap_frac * 100.0,
        top_n,
        sources_with_hits
    );

    CompletenessEstimate {
        estimate: estimate.to_string(),
        method,
        caveat: CAVEAT.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(sources: &[&str]) -> LitRecord {
        LitRecord {
            doi: None,
            pmid: None,
            title: "t".into(),
            abstract_text: None,
            authors: vec![],
            year: None,
            venue: None,
            url: None,
            source: sources.first().copied().unwrap_or("x").into(),
            source_ids: sources.iter().map(|s| format!("{s}:1")).collect(),
            cited_by_count: None,
            is_preprint: false,
            relevance: 0.0,
        }
    }

    #[test]
    fn empty_is_low() {
        let est = estimate(&[], &BTreeMap::new());
        assert_eq!(est.estimate, "low");
    }

    #[test]
    fn single_source_is_low_even_with_many_hits() {
        let recs: Vec<_> = (0..10).map(|_| rec(&["europepmc"])).collect();
        let mut id = BTreeMap::new();
        id.insert("europepmc".to_string(), 10usize);
        assert_eq!(estimate(&recs, &id).estimate, "low");
    }

    #[test]
    fn high_overlap_across_sources_is_high() {
        let recs: Vec<_> = (0..10).map(|_| rec(&["europepmc", "crossref"])).collect();
        let mut id = BTreeMap::new();
        id.insert("europepmc".to_string(), 10usize);
        id.insert("crossref".to_string(), 10usize);
        assert_eq!(estimate(&recs, &id).estimate, "high");
    }

    #[test]
    fn never_emits_a_percentage_recall_claim() {
        let est = estimate(&[rec(&["a", "b"])], &{
            let mut m = BTreeMap::new();
            m.insert("a".into(), 1);
            m.insert("b".into(), 1);
            m
        });
        assert!(!est.caveat.to_lowercase().contains("recall rate of"));
        assert!(est.caveat.to_lowercase().contains("not a measured recall"));
    }
}
