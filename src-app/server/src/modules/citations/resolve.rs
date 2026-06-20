//! Resolve a citation input → canonical CSL-JSON, and the identifier detection
//! behind `CitationInput` routing.
//!
//! Strategy (provider-agnostic, mostly doi.org-centric so it needs no
//! lit_search trait changes):
//!   * DOI    → doi.org content negotiation (`Accept: application/vnd.citationstyles.csl+json`)
//!              → CSL-JSON directly. 404/406 ⇒ `not_found` (the fabricated-DOI case).
//!   * arXiv  → arXiv DOI `10.48550/arXiv.<id>` → doi.org (DataCite hosts arXiv DOIs).
//!   * PMID/PMCID → NCBI ID-Converter → DOI → doi.org (most biomedical papers have a DOI).
//!   * title/raw  → Crossref bibliographic query → best title-match → its DOI → doi.org.
//!
//! Debug-only endpoint seams (compiled out of release via `cfg!(debug_assertions)`,
//! same pattern as lit_search's `LIT_SEARCH_*_ENDPOINT`):
//!   `CITATIONS_RESOLVER_ENDPOINT` (doi.org), `CITATIONS_IDCONV_ENDPOINT`,
//!   `CITATIONS_CROSSREF_ENDPOINT`; pair with `CITATIONS_ALLOW_LOOPBACK=1`.

use serde_json::{Value, json};

use crate::common::AppError;
use crate::modules::lit_search::dedup::normalize_doi;
use crate::utils::http_body::read_json_capped;
use crate::utils::url_validator::{OutboundUrlPolicy, build_validated_client};

use super::models::{CitationInput, VerificationStatus};
use super::verify;

const MAX_BODY: u64 = 4 * 1024 * 1024;
/// Per-request wall-clock timeout for every outbound resolve call. Without it a
/// slow/hung upstream stalls the handler — and a batch of up to 100 items
/// resolves sequentially, so one hang blocks the whole tool call. Matches the
/// per-request timeouts in `lit_search` connectors / `web_search/fetch.rs`.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const DOI_BASE: &str = "https://doi.org";
const IDCONV_BASE: &str = "https://www.ncbi.nlm.nih.gov/pmc/utils/idconv/v1.0/";
const CROSSREF_BASE: &str = "https://api.crossref.org/works";
const CSL_JSON_ACCEPT: &str = "application/vnd.citationstyles.csl+json";

/// The kind of a raw identifier string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Doi,
    Pmid,
    Pmcid,
    Arxiv,
    Unknown,
}

/// Best-effort detection of a raw identifier's kind by pattern.
pub fn detect_id_kind(raw: &str) -> IdKind {
    let s = raw.trim();
    let lower = s.to_lowercase();
    let stripped = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("doi.org/")
        .trim_start_matches("dx.doi.org/")
        .trim_start_matches("doi:")
        .trim_start_matches("arxiv:")
        .trim();

    if stripped.starts_with("pmc")
        && stripped.len() > 3
        && stripped[3..].chars().all(|c| c.is_ascii_digit())
    {
        return IdKind::Pmcid;
    }
    if stripped.starts_with("10.") && stripped.contains('/') {
        return IdKind::Doi;
    }
    if lower.starts_with("arxiv:") {
        return IdKind::Arxiv;
    }
    if is_arxiv_new(stripped) || is_arxiv_old(stripped) {
        return IdKind::Arxiv;
    }
    if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
        return IdKind::Pmid;
    }
    IdKind::Unknown
}

fn is_arxiv_new(s: &str) -> bool {
    let core = s.split('v').next().unwrap_or(s);
    let mut parts = core.split('.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), None) => {
            a.len() == 4
                && a.chars().all(|c| c.is_ascii_digit())
                && (4..=5).contains(&b.len())
                && b.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

fn is_arxiv_old(s: &str) -> bool {
    if let Some((cat, num)) = s.split_once('/') {
        let cat_ok = !cat.is_empty()
            && cat
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '.' || c == '-')
            && cat.chars().any(|c| c.is_ascii_alphabetic());
        let num_ok = num.len() >= 7 && num.chars().all(|c| c.is_ascii_digit());
        return cat_ok && num_ok;
    }
    false
}

// ─────────────────────────── pure CSL helpers ───────────────────────────

/// Extract the title from a CSL-JSON item.
pub fn csl_title(csl: &Value) -> Option<String> {
    csl.get("title")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Extract the publication year from a CSL-JSON item's `issued.date-parts`.
pub fn csl_year(csl: &Value) -> Option<i32> {
    csl.get("issued")?
        .get("date-parts")?
        .as_array()?
        .first()?
        .as_array()?
        .first()?
        .as_i64()
        .map(|y| y as i32)
}

/// Pull the identifier set out of a CSL-JSON item (DOI normalized).
pub fn csl_identifiers(csl: &Value) -> (Option<String>, Option<String>, Option<String>) {
    let doi = csl
        .get("DOI")
        .and_then(|v| v.as_str())
        .and_then(normalize_doi);
    let pmid = csl
        .get("PMID")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let pmcid = csl
        .get("PMCID")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    (doi, pmid, pmcid)
}

/// arXiv id → its DataCite DOI (e.g. `2101.12345` → `10.48550/arXiv.2101.12345`).
/// The id's case is PRESERVED — old-style ids like `math.GT/0309136` are
/// case-significant at DataCite, so lowercasing them would 404 a real preprint.
pub fn arxiv_to_doi(arxiv_id: &str) -> String {
    let id = arxiv_id
        .trim()
        .trim_start_matches("arXiv:")
        .trim_start_matches("arxiv:")
        .trim();
    format!("10.48550/arXiv.{id}")
}

/// Find a DOI inside a free-text reference string, if present. The DOI prefix
/// `10.` is ASCII, so we search `text` DIRECTLY (no lowercasing) — slicing a
/// byte offset taken from a `to_lowercase()` copy into the original `text` can
/// land mid-char and panic when a preceding char changes length under casefold
/// (e.g. `ẞ` → `ß`). Searching the original keeps every offset a valid boundary.
pub fn extract_doi_from_text(text: &str) -> Option<String> {
    let idx = text.find("10.")?;
    let tail = &text[idx..];
    let end = tail
        .find(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ')')
        .unwrap_or(tail.len());
    let candidate = &tail[..end];
    // A real DOI is `10.<registrant>/<suffix>` — require a '/' with a non-empty
    // suffix so a bare/garbage "10." (e.g. "version 10. Final") isn't mistaken
    // for a DOI, which would pre-empt the title-search fallback and misreport a
    // resolvable reference as not_found.
    match candidate.split_once('/') {
        Some((_, suffix)) if !suffix.is_empty() => normalize_doi(candidate),
        _ => None,
    }
}

// ─────────────────────────── network resolution ───────────────────────────

fn policy() -> OutboundUrlPolicy {
    #[cfg(debug_assertions)]
    if std::env::var("CITATIONS_ALLOW_LOOPBACK").is_ok() {
        return OutboundUrlPolicy::DEV_LOCAL;
    }
    OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
}

fn client() -> Result<reqwest::Client, AppError> {
    build_validated_client(policy())
        .map_err(|e| AppError::internal_error(format!("citations http client: {e}")))
}

fn endpoint(default: &str, env_key: &str) -> String {
    #[cfg(debug_assertions)]
    if let Ok(u) = std::env::var(env_key)
        && !u.trim().is_empty()
    {
        return u;
    }
    let _ = env_key;
    default.to_string()
}

/// Resolve a DOI to CSL-JSON via content negotiation. `Ok(None)` = the DOI does
/// not resolve (fabricated / wrong). `Err` = transient/network failure.
pub async fn resolve_doi_csl(doi: &str) -> Result<Option<Value>, AppError> {
    let base = endpoint(DOI_BASE, "CITATIONS_RESOLVER_ENDPOINT");
    // Build the URL via path segments so each part of the DOI is percent-encoded
    // (the `/` structure is preserved by splitting on it). A raw `format!` would
    // mangle DOIs with reserved chars; the validated client's GuardingResolver is
    // the actual SSRF control, this is correctness. doi is already normalized.
    let mut url = reqwest::Url::parse(&base)
        .map_err(|e| AppError::internal_error(format!("bad resolver base: {e}")))?;
    {
        let mut segs = url
            .path_segments_mut()
            .map_err(|_| AppError::internal_error("resolver base cannot be a base"))?;
        segs.pop_if_empty();
        segs.extend(doi.split('/'));
    }
    let resp = client()?
        .get(url)
        .header("Accept", CSL_JSON_ACCEPT)
        .header("User-Agent", format!("ziee/{}", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("citations: doi.org request failed: {e}");
            AppError::internal_error("doi resolution request failed")
        })?;
    let status = resp.status();
    if status.as_u16() == 404 || status.as_u16() == 406 || status.as_u16() == 204 {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(AppError::internal_error(format!(
            "doi.org returned HTTP {status}"
        )));
    }
    let csl: Value = read_json_capped(resp, MAX_BODY).await?;
    // A valid CSL-JSON record is an object; a stray array/scalar (a
    // misbehaving resolver) is not a usable record — treat as not-found rather
    // than storing a shape that would later break pandoc export.
    if !csl.is_object() {
        return Ok(None);
    }
    Ok(Some(csl))
}

/// Outcome of an NCBI ID-Converter lookup — distinguishes "no such record"
/// (truly not found → fabricated) from "record exists but has no DOI"
/// (legitimate for older PubMed entries → unverified, NOT not_found).
enum IdconvResult {
    /// A record exists and carries a DOI.
    Doi(String),
    /// A record exists but has no DOI registered.
    RecordNoDoi,
    /// No record for this id.
    NotFound,
}

/// PMID/PMCID → ID-Converter result. NCBI returns `records[].errmsg`/`status`
/// for a missing id and a record (possibly without `doi`) for a real one.
async fn idconv_lookup(id: &str) -> Result<IdconvResult, AppError> {
    let base = endpoint(IDCONV_BASE, "CITATIONS_IDCONV_ENDPOINT");
    let resp = client()?
        .get(&base)
        .query(&[("ids", id), ("format", "json"), ("tool", "ziee")])
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("citations: idconv request failed: {e}");
            AppError::internal_error("pmid resolution request failed")
        })?;
    if resp.status().as_u16() == 404 {
        return Ok(IdconvResult::NotFound);
    }
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "idconv returned HTTP {}",
            resp.status()
        )));
    }
    let v: Value = read_json_capped(resp, MAX_BODY).await?;
    let record = v
        .get("records")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first());
    match record {
        // A record with an explicit error / status is "not found".
        Some(rec) if rec.get("errmsg").is_some() || rec.get("status").is_some() => {
            Ok(IdconvResult::NotFound)
        }
        Some(rec) => match rec.get("doi").and_then(|d| d.as_str()).and_then(normalize_doi) {
            Some(doi) => Ok(IdconvResult::Doi(doi)),
            None => Ok(IdconvResult::RecordNoDoi),
        },
        None => Ok(IdconvResult::NotFound),
    }
}

/// Free-text title → the best-matching record's DOI via a Crossref bibliographic
/// query (then resolved to CSL-JSON by the caller).
async fn crossref_title_to_doi(title: &str) -> Result<Option<String>, AppError> {
    let base = endpoint(CROSSREF_BASE, "CITATIONS_CROSSREF_ENDPOINT");
    let resp = client()?
        .get(&base)
        .query(&[
            ("query.bibliographic", title),
            ("rows", "5"),
            ("select", "DOI,title"),
        ])
        .header("User-Agent", format!("ziee/{}", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("citations: crossref title query failed: {e}");
            AppError::internal_error("title search request failed")
        })?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "crossref returned HTTP {}",
            resp.status()
        )));
    }
    let v: Value = read_json_capped(resp, MAX_BODY).await?;
    let items = v
        .get("message")
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    for it in items {
        let cand_title = it
            .get("title")
            .and_then(|t| t.as_array())
            .and_then(|a| a.first())
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if verify::title_matches(title, cand_title) {
            if let Some(doi) = it.get("DOI").and_then(|d| d.as_str()).and_then(normalize_doi) {
                return Ok(Some(doi));
            }
        }
    }
    Ok(None)
}

/// The resolved outcome for one `CitationInput`.
pub struct Resolved {
    pub csl: Option<Value>,
    pub status: VerificationStatus,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub arxiv_id: Option<String>,
    pub mismatch_fields: Vec<String>,
}

impl Resolved {
    fn unverified(csl: Option<Value>) -> Self {
        let (doi, pmid, pmcid) = csl.as_ref().map(csl_identifiers).unwrap_or((None, None, None));
        Self {
            csl,
            status: VerificationStatus::Unverified,
            doi,
            pmid,
            pmcid,
            arxiv_id: None,
            mismatch_fields: vec![],
        }
    }
}

/// Resolve + verify one input. The claimed title (input.title or the title in a
/// supplied csl) is cross-checked against the resolved record → `mismatch`.
pub async fn resolve_input(input: &CitationInput) -> Result<Resolved, AppError> {
    // 1. Figure out the best identifier / search term.
    let claimed_title = input
        .title
        .clone()
        .or_else(|| input.csl.as_ref().and_then(csl_title));

    // Explicit/auto identifier.
    let id_str = input.id.clone().or_else(|| {
        input
            .raw
            .as_ref()
            .and_then(|r| extract_doi_from_text(r))
    });

    if let Some(id) = id_str.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let kind = match input.kind.as_deref() {
            Some("doi") => IdKind::Doi,
            Some("pmid") => IdKind::Pmid,
            Some("pmcid") => IdKind::Pmcid,
            Some("arxiv") => IdKind::Arxiv,
            _ => detect_id_kind(id),
        };
        return resolve_by_kind(id, kind, claimed_title.as_deref()).await;
    }

    // 2. A supplied CSL-JSON with its own DOI → verify it; otherwise store as-is.
    if let Some(csl) = &input.csl {
        let (doi, _, _) = csl_identifiers(csl);
        if let Some(doi) = doi {
            return verify_against_doi(&doi, claimed_title.as_deref()).await;
        }
        return Ok(Resolved::unverified(Some(csl.clone())));
    }

    // 3. Free-text title search.
    if let Some(title) = claimed_title.as_deref() {
        if let Some(doi) = crossref_title_to_doi(title).await? {
            return verify_against_doi(&doi, Some(title)).await;
        }
        // No confident match → unverified (NOT not_found — a search miss is not
        // proof the work doesn't exist). Synthesize CSL from ALL the fields the
        // caller supplied (title + authors + year + journal) so the stored
        // record + its dedup fingerprint + citation_key aren't title-only.
        return Ok(Resolved::unverified(Some(csl_from_input(input, title))));
    }

    Err(AppError::bad_request(
        "CITATIONS_EMPTY_INPUT",
        "each item needs at least one of id / title / csl / raw",
    ))
}

/// Build a CSL-JSON item from the caller-supplied fields when nothing resolved.
/// Keeps title + authors (→ CSL `author[].family`) + year (→ `issued`) + journal
/// (→ `container-title`) so the stored record, its dedup fingerprint, and its
/// citation_key reflect everything the caller knew — not just the title.
fn csl_from_input(input: &CitationInput, title: &str) -> Value {
    let mut csl = json!({ "type": "article-journal", "title": title });
    let obj = csl.as_object_mut().expect("json object literal");
    if let Some(authors) = &input.authors {
        let arr: Vec<Value> = authors
            .iter()
            .filter(|a| !a.trim().is_empty())
            .map(|a| json!({ "family": a }))
            .collect();
        if !arr.is_empty() {
            obj.insert("author".into(), Value::Array(arr));
        }
    }
    if let Some(year) = input.year {
        obj.insert("issued".into(), json!({ "date-parts": [[year]] }));
    }
    if let Some(journal) = &input.journal {
        if !journal.trim().is_empty() {
            obj.insert("container-title".into(), json!(journal));
        }
    }
    csl
}

async fn resolve_by_kind(
    id: &str,
    kind: IdKind,
    claimed_title: Option<&str>,
) -> Result<Resolved, AppError> {
    match kind {
        IdKind::Doi => {
            let doi = normalize_doi(id).unwrap_or_else(|| id.to_string());
            verify_against_doi(&doi, claimed_title).await
        }
        IdKind::Arxiv => {
            let doi = arxiv_to_doi(id);
            let mut r = verify_against_doi(&doi, claimed_title).await?;
            r.arxiv_id = Some(
                id.trim_start_matches("arXiv:")
                    .trim_start_matches("arxiv:")
                    .trim()
                    .to_string(),
            );
            Ok(r)
        }
        IdKind::Pmid | IdKind::Pmcid => {
            let pmcol = if kind == IdKind::Pmcid {
                Some(id.to_uppercase())
            } else {
                None
            };
            let pmid = if kind == IdKind::Pmid {
                Some(id.to_string())
            } else {
                None
            };
            match idconv_lookup(id).await? {
                IdconvResult::Doi(doi) => {
                    let mut r = verify_against_doi(&doi, claimed_title).await?;
                    r.pmid = r.pmid.or(pmid);
                    r.pmcid = r.pmcid.or(pmcol);
                    Ok(r)
                }
                // A real record with no DOI: the id IS real (it resolved to a
                // record), we just can't cross-check it via doi.org → unverified,
                // NOT not_found. Store it with its PMID/PMCID.
                IdconvResult::RecordNoDoi => Ok(Resolved {
                    csl: Some(json!({ "type": "article-journal" })),
                    status: VerificationStatus::Unverified,
                    doi: None,
                    pmid,
                    pmcid: pmcol,
                    arxiv_id: None,
                    mismatch_fields: vec![],
                }),
                // No record at all → fabricated / wrong id.
                IdconvResult::NotFound => Ok(Resolved {
                    csl: None,
                    status: VerificationStatus::NotFound,
                    doi: None,
                    pmid,
                    pmcid: pmcol,
                    arxiv_id: None,
                    mismatch_fields: vec![],
                }),
            }
        }
        IdKind::Unknown => {
            // Not an identifier — treat as a free-text title search.
            if let Some(t) = claimed_title.or(Some(id)) {
                if let Some(doi) = crossref_title_to_doi(t).await? {
                    return verify_against_doi(&doi, Some(t)).await;
                }
            }
            Ok(Resolved::unverified(Some(
                json!({ "type": "article-journal", "title": id }),
            )))
        }
    }
}

/// Resolve a DOI and decide verified/mismatch/not_found, cross-checking the
/// claimed title against the resolved record's title when provided.
async fn verify_against_doi(
    doi: &str,
    claimed_title: Option<&str>,
) -> Result<Resolved, AppError> {
    match resolve_doi_csl(doi).await? {
        None => Ok(Resolved {
            csl: None,
            status: VerificationStatus::NotFound,
            doi: Some(doi.to_string()),
            pmid: None,
            pmcid: None,
            arxiv_id: None,
            mismatch_fields: vec![],
        }),
        Some(csl) => {
            let (rdoi, pmid, pmcid) = csl_identifiers(&csl);
            let resolved_title = csl_title(&csl);
            let (status, mismatch) = match (claimed_title, &resolved_title) {
                (Some(claimed), Some(resolved)) if !verify::title_matches(claimed, resolved) => {
                    (VerificationStatus::Mismatch, vec!["title".to_string()])
                }
                _ => (VerificationStatus::Verified, vec![]),
            };
            Ok(Resolved {
                doi: rdoi.or_else(|| Some(doi.to_string())),
                pmid,
                pmcid,
                arxiv_id: None,
                csl: Some(csl),
                status,
                mismatch_fields: mismatch,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_doi_variants() {
        assert_eq!(detect_id_kind("10.1038/s41586-021-1"), IdKind::Doi);
        assert_eq!(
            detect_id_kind("https://doi.org/10.1126/science.169.3946.635"),
            IdKind::Doi
        );
        assert_eq!(detect_id_kind("DOI:10.1000/xyz"), IdKind::Doi);
    }

    #[test]
    fn detects_pmid_and_pmcid() {
        assert_eq!(detect_id_kind("34121113"), IdKind::Pmid);
        assert_eq!(detect_id_kind("PMC8079011"), IdKind::Pmcid);
        assert_eq!(detect_id_kind("pmc8079011"), IdKind::Pmcid);
    }

    #[test]
    fn detects_arxiv() {
        assert_eq!(detect_id_kind("2101.12345"), IdKind::Arxiv);
        assert_eq!(detect_id_kind("2101.12345v2"), IdKind::Arxiv);
        assert_eq!(detect_id_kind("arXiv:2101.12345"), IdKind::Arxiv);
        assert_eq!(detect_id_kind("math.GT/0309136"), IdKind::Arxiv);
    }

    #[test]
    fn unknown_for_freetext() {
        assert_eq!(detect_id_kind("some paper title"), IdKind::Unknown);
        assert_eq!(detect_id_kind(""), IdKind::Unknown);
    }

    #[test]
    fn arxiv_doi_construction() {
        assert_eq!(arxiv_to_doi("2101.12345"), "10.48550/arXiv.2101.12345");
        assert_eq!(arxiv_to_doi("arXiv:2101.12345"), "10.48550/arXiv.2101.12345");
        // Old-style ids keep their case (case-significant at DataCite).
        assert_eq!(arxiv_to_doi("math.GT/0309136"), "10.48550/arXiv.math.GT/0309136");
    }

    #[test]
    fn csl_extractors() {
        let csl = json!({
            "title": "A Title",
            "DOI": "10.1038/ABC",
            "PMID": "123",
            "issued": { "date-parts": [[2021, 6, 1]] }
        });
        assert_eq!(csl_title(&csl).as_deref(), Some("A Title"));
        assert_eq!(csl_year(&csl), Some(2021));
        let (doi, pmid, pmcid) = csl_identifiers(&csl);
        assert_eq!(doi.as_deref(), Some("10.1038/abc")); // normalized lowercase
        assert_eq!(pmid.as_deref(), Some("123"));
        assert_eq!(pmcid, None);
    }

    #[test]
    fn extract_doi_from_reference_string() {
        assert_eq!(
            extract_doi_from_text("Smith J. Title. Journal. 2020. doi:10.1038/s41586-020-1, retrieved")
                .as_deref(),
            Some("10.1038/s41586-020-1")
        );
        assert_eq!(extract_doi_from_text("no doi here"), None);
        // A bare "10." with no /suffix is NOT a DOI (must not pre-empt title search).
        assert_eq!(extract_doi_from_text("see version 10. Final draft"), None);
        assert_eq!(extract_doi_from_text("10."), None);
    }

    #[test]
    fn extract_doi_from_text_handles_casefold_changing_unicode() {
        // Regression: a preceding char whose length changes under to_lowercase
        // (ẞ U+1E9E 3 bytes → ß 2 bytes) must NOT cause a non-char-boundary panic.
        assert_eq!(
            extract_doi_from_text("ẞ€ ref doi:10.1234/x"),
            Some("10.1234/x".to_string())
        );
        // İ (U+0130) lowercases to two code points — must also not panic/mis-slice.
        assert_eq!(
            extract_doi_from_text("İstanbul study 10.5555/abc end"),
            Some("10.5555/abc".to_string())
        );
    }
}
