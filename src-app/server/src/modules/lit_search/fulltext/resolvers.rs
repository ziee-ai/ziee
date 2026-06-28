//! Open-access full-text resolution per identifier.
//!
//! OA-only (no scraping of paywalled content). v1 resolvers:
//!   * PMID / PMCID → Europe PMC `fullTextXML` (JATS → text).
//!   * DOI          → Unpaywall (best OA PDF) → pdfium text extraction.
//!   * arXiv id     → arXiv PDF → pdfium text extraction.
//! CORE/S2 full-text-by-id are deferred (their search responses already surface
//! abstracts; native full-text-by-id is a follow-up).
//!
//! External OA PDF/XML hosts are reached through the connector SSRF policy
//! (`PUBLIC_HTTP_OR_HTTPS`); arbitrary OA-PDF URLs from Unpaywall are validated
//! by that client's DNS/redirect guard.

use std::time::Duration;

use serde::Deserialize;

use super::super::connectors::{
    MAX_BODY_BYTES, build_client, endpoint, read_bytes_capped, read_text_capped,
};
use super::cache::{STATUS_FULL_TEXT, STATUS_NOT_FOUND, STATUS_NOT_OA};

/// The identifiers a paper may be addressed by.
#[derive(Debug, Clone, Default)]
pub struct PaperIds {
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmcid: Option<String>,
    pub arxiv_id: Option<String>,
}

/// Detect which kind of identifier `raw` is.
pub fn parse_id(raw: &str) -> PaperIds {
    let s = raw.trim();
    let lower = s.to_lowercase();
    let mut ids = PaperIds::default();

    if let Some(doi) = super::super::dedup::normalize_doi(s) {
        ids.doi = Some(doi);
        return ids;
    }
    if lower.starts_with("pmc") {
        ids.pmcid = Some(s.to_uppercase());
        return ids;
    }
    if let Some(rest) = lower.strip_prefix("pmid:") {
        let r = rest.trim();
        if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()) {
            ids.pmid = Some(r.to_string());
            return ids;
        }
    }
    if let Some(rest) = lower.strip_prefix("arxiv:") {
        ids.arxiv_id = Some(rest.trim().to_string());
        return ids;
    }
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        ids.pmid = Some(s.to_string());
        return ids;
    }
    // Fall back to treating it as an arXiv id (e.g. 2201.00001 / cs/0112017).
    ids.arxiv_id = Some(s.to_string());
    ids
}

/// A best id label for display / view-link naming.
pub fn display_id(ids: &PaperIds) -> String {
    ids.doi
        .clone()
        .or_else(|| ids.pmcid.clone())
        .or_else(|| ids.pmid.as_ref().map(|p| format!("pmid{p}")))
        .or_else(|| ids.arxiv_id.as_ref().map(|a| format!("arxiv{a}")))
        .unwrap_or_else(|| "paper".to_string())
}

pub struct Resolved {
    pub status: String,
    pub text: Option<String>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub version: Option<String>,
}

impl Resolved {
    fn not_oa() -> Self {
        Self { status: STATUS_NOT_OA.into(), text: None, source: None, license: None, version: None }
    }
    fn not_found() -> Self {
        Self { status: STATUS_NOT_FOUND.into(), text: None, source: None, license: None, version: None }
    }
    fn full(text: String, source: &str) -> Self {
        Self {
            status: STATUS_FULL_TEXT.into(),
            text: Some(text),
            source: Some(source.to_string()),
            license: None,
            version: None,
        }
    }
}

const EPMC_BASE: &str = "https://www.ebi.ac.uk/europepmc/webservices/rest";

/// Percent-encode a model-supplied id for safe interpolation into a URL PATH
/// segment. Keeps `/` (DOIs and old-style arXiv ids like `math/0309136` carry a
/// literal slash) and the unreserved set; encodes everything else — crucially
/// the `#`/`?`/space/`<`/`>` chars that are legal in DOIs but would otherwise be
/// parsed as query/fragment delimiters and silently fetch the wrong paper.
pub(crate) fn encode_path_id(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Resolve OA full text for the given ids. `email` (from a connector's mailto)
/// is required for Unpaywall; without it the DOI→PDF path is skipped.
pub async fn resolve(ids: &PaperIds, email: Option<&str>, timeout: Duration) -> Resolved {
    let client = match build_client() {
        Ok(c) => c,
        Err(_) => return Resolved::not_found(),
    };

    // 1) Europe PMC fullTextXML — the OA full-text corpus is keyed by PMCID
    //    under the `PMC` source. For a bare PMID we first convert PMID→PMCID via
    //    EPMC search (the `MED`/PMID fullTextXML path does NOT exist and always
    //    404s), then fetch under `PMC`.
    if let Some(pmcid) = &ids.pmcid {
        if let Some(text) = epmc_fulltext(&client, "PMC", pmcid, timeout).await {
            return Resolved::full(text, "europepmc");
        }
    }
    if let Some(pmid) = &ids.pmid {
        if let Some(pmcid) = epmc_pmid_to_pmcid(&client, pmid, timeout).await
            && let Some(text) = epmc_fulltext(&client, "PMC", &pmcid, timeout).await
        {
            return Resolved::full(text, "europepmc");
        }
    }

    // 2) arXiv PDF.
    if let Some(arxiv) = &ids.arxiv_id {
        let url = format!("https://arxiv.org/pdf/{}.pdf", encode_path_id(arxiv));
        if let Some(text) = pdf_to_text(&client, &url, timeout).await {
            return Resolved::full(text, "arxiv");
        }
    }

    // 3) Unpaywall (DOI → best OA PDF) → pdfium. Requires a contact email.
    if let Some(doi) = &ids.doi {
        if let Some(mail) = email {
            if let Some(pdf_url) = unpaywall_pdf(&client, doi, mail, timeout).await
                && let Some(text) = pdf_to_text(&client, &pdf_url, timeout).await
            {
                return Resolved::full(text, "unpaywall");
            }
            // Checked Unpaywall, no OA PDF → genuinely not open access.
            return Resolved::not_oa();
        }
        // DOI present but no contact email configured: Unpaywall can't be queried,
        // so we did NOT determine OA status. Report not_found (re-resolvable once a
        // Crossref/PubMed mailto is set) rather than mislabeling it as paywalled.
        return Resolved::not_found();
    }

    // We tried the available id paths (EPMC/arXiv) and found no OA full text.
    if ids.pmid.is_some() || ids.pmcid.is_some() || ids.arxiv_id.is_some() {
        Resolved::not_oa()
    } else {
        Resolved::not_found()
    }
}

/// Resolve a PMID to its PMCID (if the paper is in the Europe PMC OA corpus) via
/// the EPMC search API. Returns None when the PMID has no PMC full-text copy.
async fn epmc_pmid_to_pmcid(
    client: &reqwest::Client,
    pmid: &str,
    timeout: Duration,
) -> Option<String> {
    #[derive(Deserialize)]
    struct SearchResp {
        #[serde(rename = "resultList")]
        result_list: Option<ResultList>,
    }
    #[derive(Deserialize)]
    struct ResultList {
        #[serde(default)]
        result: Vec<ResultItem>,
    }
    #[derive(Deserialize)]
    struct ResultItem {
        pmcid: Option<String>,
    }
    let base = endpoint(EPMC_BASE, "LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT");
    let url = format!("{base}/search");
    let resp = client
        .get(&url)
        .query(&[
            ("query", format!("ext_id:{pmid} AND SRC:MED").as_str()),
            ("format", "json"),
            ("resultType", "lite"),
            ("pageSize", "1"),
        ])
        .timeout(timeout)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = read_bytes_capped(resp, MAX_BODY_BYTES).await.ok()?;
    let parsed: SearchResp = serde_json::from_slice(&bytes).ok()?;
    parsed
        .result_list?
        .result
        .into_iter()
        .find_map(|r| r.pmcid.filter(|p| !p.trim().is_empty()))
}

async fn epmc_fulltext(
    client: &reqwest::Client,
    source: &str,
    id: &str,
    timeout: Duration,
) -> Option<String> {
    let base = endpoint(EPMC_BASE, "LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT");
    let url = format!("{base}/{source}/{}/fullTextXML", encode_path_id(id));
    let resp = client.get(&url).timeout(timeout).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let xml = read_text_capped(resp, MAX_BODY_BYTES).await.ok()?;
    let text = strip_xml_tags(&xml);
    (!text.trim().is_empty()).then_some(text)
}

#[derive(Deserialize)]
struct UnpaywallResponse {
    best_oa_location: Option<UnpaywallLocation>,
}

#[derive(Deserialize)]
struct UnpaywallLocation {
    #[serde(default)]
    url_for_pdf: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

async fn unpaywall_pdf(
    client: &reqwest::Client,
    doi: &str,
    email: &str,
    timeout: Duration,
) -> Option<String> {
    let url = format!("https://api.unpaywall.org/v2/{}", encode_path_id(doi));
    let resp = client
        .get(&url)
        .query(&[("email", email)])
        .timeout(timeout)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = read_bytes_capped(resp, MAX_BODY_BYTES).await.ok()?;
    let parsed: UnpaywallResponse = serde_json::from_slice(&bytes).ok()?;
    let loc = parsed.best_oa_location?;
    // Prefer the direct PDF link; `url` is often a landing PAGE (HTML), which
    // pdf_to_text rejects via the %PDF magic-byte guard below.
    loc.url_for_pdf.or(loc.url).filter(|u| !u.is_empty())
}

/// Fetch a PDF and extract its text via the shared pdfium runtime (reused from
/// the file module). Best-effort: any failure returns None (→ not_open_access).
async fn pdf_to_text(client: &reqwest::Client, url: &str, timeout: Duration) -> Option<String> {
    let resp = client.get(url).timeout(timeout).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = read_bytes_capped(resp, MAX_BODY_BYTES).await.ok()?;

    // Guard against HTML landing pages (Unpaywall's `url` can be a page, not a
    // file): only feed real PDFs to pdfium. `%PDF` may follow a few leading bytes.
    let looks_like_pdf = bytes
        .windows(5)
        .take(1024)
        .any(|w| w == b"%PDF-");
    if !looks_like_pdf {
        return None;
    }

    let pdfium = crate::modules::file::utils::pdfium::init_pdfium().ok()?;
    let document = pdfium.load_pdf_from_byte_slice(&bytes, None).ok()?;
    let mut out = String::new();
    let pages = document.pages();
    for i in 0..pages.len() {
        if let Ok(page) = pages.get(i)
            && let Ok(text) = page.text()
        {
            out.push_str(&text.all());
            out.push('\n');
        }
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then_some(out)
}

/// Strip XML/JATS tags to plain text (best-effort), collapsing whitespace runs.
fn strip_xml_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_id_detects_kinds() {
        assert_eq!(parse_id("10.1/abc").doi.as_deref(), Some("10.1/abc"));
        assert_eq!(parse_id("PMC123456").pmcid.as_deref(), Some("PMC123456"));
        assert_eq!(parse_id("31634902").pmid.as_deref(), Some("31634902"));
        assert_eq!(parse_id("arXiv:2201.00001").arxiv_id.as_deref(), Some("2201.00001"));
        assert_eq!(parse_id("2201.00001").arxiv_id.as_deref(), Some("2201.00001"));
        assert_eq!(parse_id("https://doi.org/10.1/X").doi.as_deref(), Some("10.1/x"));
    }

    #[test]
    fn strips_jats_tags() {
        let xml = "<article><body><p>Hello <b>world</b></p></body></article>";
        assert_eq!(strip_xml_tags(xml), "Hello world");
    }

    #[test]
    fn encode_path_id_keeps_slash_encodes_delimiters() {
        // Common DOI: untouched (slash + unreserved preserved).
        assert_eq!(encode_path_id("10.1038/nature12373"), "10.1038/nature12373");
        // Reserved/unsafe chars that would break path parsing are encoded.
        assert_eq!(encode_path_id("10.1000/abc#def"), "10.1000/abc%23def");
        assert_eq!(encode_path_id("10.1/a?b c"), "10.1/a%3Fb%20c");
        assert_eq!(encode_path_id("10.1/<x>"), "10.1/%3Cx%3E");
        // Old-style arXiv id keeps its slash.
        assert_eq!(encode_path_id("math/0309136"), "math/0309136");
    }
}
