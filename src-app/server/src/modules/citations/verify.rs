//! Verification: confirm a reference resolves to a real record and that its
//! title matches the canonical title from the resolver. The title-match
//! heuristic is adopted from the user's `doi-to-ref.js::verifyTitleWithDoi`.
//!
//! Status semantics (see `models::VerificationStatus`):
//!   * `verified`   — id resolved + title matches (or a confident title-search hit)
//!   * `mismatch`   — id resolved but to a *different* paper
//!   * `not_found`  — a *supplied id* failed to resolve (fabricated)
//!   * `unverified` — no id to check (legitimate id-less item) / not yet checked

/// Normalize a title for comparison: strip braces/backslashes, drop non-word
/// punctuation, lowercase, collapse whitespace.
pub fn normalize_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        let mapped = if ch == '{' || ch == '}' || ch == '\\' {
            // drop
            continue;
        } else if ch.is_alphanumeric() {
            prev_space = false;
            ch.to_ascii_lowercase()
        } else {
            // any punctuation/whitespace → single space
            if prev_space {
                continue;
            }
            prev_space = true;
            ' '
        };
        out.push(mapped);
    }
    out.trim().to_string()
}

/// Does `stored` plausibly refer to the same work as the `resolved` title?
/// Match if the normalized stored title is a substring of the resolved title,
/// OR ≥ 60% of the stored title's words (length > 2) appear in the resolved
/// title. Lenient by design (avoids false `mismatch`).
pub fn title_matches(stored: &str, resolved: &str) -> bool {
    let n_stored = normalize_title(stored);
    let n_resolved = normalize_title(resolved);
    if n_stored.is_empty() || n_resolved.is_empty() {
        return false;
    }
    if n_resolved.contains(&n_stored) || n_stored.contains(&n_resolved) {
        return true;
    }
    let words: Vec<&str> = n_stored.split(' ').filter(|w| w.len() > 2).collect();
    if words.is_empty() {
        return false;
    }
    let matched = words.iter().filter(|w| n_resolved.contains(**w)).count();
    (matched as f32) / (words.len() as f32) >= 0.60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_braces_punct_and_case() {
        assert_eq!(
            normalize_title("{CRISPR} interference, in Plants!"),
            "crispr interference in plants"
        );
        assert_eq!(normalize_title("  Foo —  Bar  "), "foo bar");
    }

    #[test]
    fn substring_match() {
        assert!(title_matches(
            "CRISPR interference",
            "CRISPR interference in plant gene regulation"
        ));
    }

    #[test]
    fn word_overlap_match_above_threshold() {
        // 4/5 long words present → ≥60%.
        assert!(title_matches(
            "Genome wide association study maize",
            "A genome-wide association study of maize kernels"
        ));
    }

    #[test]
    fn mismatch_below_threshold() {
        assert!(!title_matches(
            "Quantum entanglement photonics",
            "CRISPR interference in plant gene regulation"
        ));
    }

    #[test]
    fn empty_titles_do_not_match() {
        assert!(!title_matches("", "anything"));
        assert!(!title_matches("anything", ""));
    }
}
