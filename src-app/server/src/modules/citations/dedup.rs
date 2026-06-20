//! Dedup helpers for identifier-less entries + citation-key generation.
//!
//! Dedup priority: normalized DOI (reuse `lit_search::dedup::normalize_doi`) >
//! PMID exact > the `dedup_fingerprint` below (identifier-less). Exact
//! fingerprint → auto-link (race-safe via the partial unique index); fuzzy
//! near-match → user review (never auto-merged).

use super::verify::normalize_title;

/// Composite fingerprint for an identifier-less entry: `normTitle|surname|year`.
/// Stored in `bibliography_entries.dedup_fingerprint` (NULL when a DOI/PMID
/// exists). Two entries with the same fingerprint are the same work (exact bar).
pub fn fingerprint(title: &str, first_author_surname: Option<&str>, year: Option<i32>) -> String {
    let t = normalize_title(title);
    let a = first_author_surname
        .map(|s| normalize_title(s))
        .unwrap_or_default();
    let y = year.map(|y| y.to_string()).unwrap_or_default();
    format!("{t}|{a}|{y}")
}

/// Pull the first author's family/surname out of a CSL-JSON item.
pub fn first_author_surname(csl: &serde_json::Value) -> Option<String> {
    let authors = csl.get("author")?.as_array()?;
    let first = authors.first()?;
    // CSL author: { family, given } or { literal }
    if let Some(family) = first.get("family").and_then(|v| v.as_str()) {
        return Some(family.to_string());
    }
    if let Some(literal) = first.get("literal").and_then(|v| v.as_str()) {
        // "Surname, Given" or "Given Surname" → take the last whitespace token
        return literal
            .split([',', ' '])
            .map(|s| s.trim())
            .find(|s| !s.is_empty())
            .map(|s| s.to_string());
    }
    None
}

/// Slugged surname for a citation key: lowercase ascii letters only.
fn slug_surname(surname: &str) -> String {
    let s: String = surname
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_lowercase();
    if s.is_empty() { "anon".to_string() } else { s }
}

/// The un-suffixed `surnameYEAR` base of a citation key. Used both to generate
/// the key and to build the `LIKE` prefix for the collision query.
pub fn citation_key_base(surname: Option<&str>, year: Option<i32>) -> String {
    format!(
        "{}{}",
        slug_surname(surname.unwrap_or("anon")),
        year.map(|y| y.to_string()).unwrap_or_default()
    )
}

/// Generate a `surnameYEAR` citation key, suffixing with a/b/c… on collision
/// with `existing` keys (already-used keys for this user).
pub fn gen_citation_key(
    surname: Option<&str>,
    year: Option<i32>,
    existing: &[String],
) -> String {
    let base = citation_key_base(surname, year);
    if !existing.iter().any(|k| k == &base) {
        return base;
    }
    for suffix in b'a'..=b'z' {
        let candidate = format!("{base}{}", suffix as char);
        if !existing.iter().any(|k| k == &candidate) {
            return candidate;
        }
    }
    // Exhausted a–z: fall back to a numeric suffix.
    let mut n = 1;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.iter().any(|k| k == &candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fingerprint_is_stable_and_normalized() {
        let a = fingerprint("{CRISPR} Interference!", Some("Smith"), Some(2021));
        let b = fingerprint("crispr interference", Some("smith"), Some(2021));
        assert_eq!(a, b);
        assert_eq!(a, "crispr interference|smith|2021");
    }

    #[test]
    fn first_author_from_family_and_literal() {
        let csl = json!({ "author": [{ "family": "Doe", "given": "A." }] });
        assert_eq!(first_author_surname(&csl).as_deref(), Some("Doe"));
        let csl2 = json!({ "author": [{ "literal": "World Health Organization" }] });
        assert_eq!(first_author_surname(&csl2).as_deref(), Some("World"));
        let csl3 = json!({ "title": "no authors" });
        assert_eq!(first_author_surname(&csl3), None);
    }

    #[test]
    fn citation_key_collision_suffixing() {
        assert_eq!(gen_citation_key(Some("Smith"), Some(2021), &[]), "smith2021");
        let existing = vec!["smith2021".to_string()];
        assert_eq!(
            gen_citation_key(Some("Smith"), Some(2021), &existing),
            "smith2021a"
        );
        let existing = vec!["smith2021".to_string(), "smith2021a".to_string()];
        assert_eq!(
            gen_citation_key(Some("Smith"), Some(2021), &existing),
            "smith2021b"
        );
    }

    #[test]
    fn anon_key_when_no_surname() {
        assert_eq!(gen_citation_key(None, None, &[]), "anon");
    }
}
