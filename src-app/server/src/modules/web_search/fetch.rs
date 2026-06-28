//! Page fetch: an untrusted, model-supplied URL → clean markdown.
//!
//! SSRF is enforced via the shared `url_validator`: the default policy is
//! public http+https only (blocks loopback / RFC1918 / link-local / cloud
//! IMDS), and the validated client re-checks every redirect hop. A DEBUG-only
//! env seam (`WEB_SEARCH_FETCH_ALLOW_LOOPBACK`) relaxes the policy to
//! `DEV_LOCAL` so integration tests can fetch a 127.0.0.1 fixture — it is
//! compiled out of release builds via `cfg!(debug_assertions)` and cannot be
//! set in production (same pattern as `CODE_SANDBOX_ROOTFS_MIRROR`).

use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;

use crate::common::AppError;
use crate::utils::url_validator::{OutboundUrlPolicy, build_validated_client, validate_outbound_url};

/// Result of a page fetch, returned to the model via `structuredContent`.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FetchedPage {
    /// The URL requested.
    pub url: String,
    /// The final URL after any redirects.
    pub final_url: String,
    /// Extracted page title (may be empty).
    pub title: String,
    /// Main content as markdown, truncated to the configured char cap.
    pub content: String,
    /// True if `content` was truncated at the char cap.
    pub truncated: bool,
    /// Raw response size in bytes (before extraction).
    pub byte_count: u64,
}

/// SSRF policy for untrusted page fetches. DEBUG-only env opt-in relaxes it for
/// loopback test fixtures; release builds always use the public-only policy.
fn fetch_policy() -> OutboundUrlPolicy {
    #[cfg(debug_assertions)]
    {
        if std::env::var("WEB_SEARCH_FETCH_ALLOW_LOOPBACK").is_ok() {
            return OutboundUrlPolicy::DEV_LOCAL;
        }
    }
    OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS
}

pub async fn fetch_url(
    url: &str,
    max_bytes: u64,
    max_chars: usize,
    timeout_secs: u64,
) -> Result<FetchedPage, AppError> {
    let policy = fetch_policy();
    // Pre-flight validate the untrusted URL; the built client's redirect policy
    // re-validates every hop under the SAME policy.
    validate_outbound_url(url, &policy)
        .map_err(|e| AppError::bad_request("WEB_FETCH_BLOCKED_URL", format!("url rejected: {e}")))?;
    let client = build_validated_client(policy)
        .map_err(|e| AppError::internal_error(format!("failed to build http client: {e}")))?;

    let resp = client
        .get(url)
        .header("User-Agent", concat!("ziee/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "text/html,application/xhtml+xml,text/plain;q=0.9")
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|e| AppError::bad_request("WEB_FETCH_FAILED", format!("fetch failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::bad_request(
            "WEB_FETCH_HTTP_ERROR",
            format!("fetch returned HTTP {}", resp.status()),
        ));
    }

    let final_url = resp.url().to_string();

    // Early reject by Content-Length when the server advertises it.
    if let Some(len) = resp.content_length()
        && len > max_bytes
    {
        return Err(AppError::bad_request(
            "WEB_FETCH_TOO_LARGE",
            format!("response is {len} bytes, exceeds cap of {max_bytes}"),
        ));
    }

    // Stream with a hard byte cap (Content-Length may be absent or lie).
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| AppError::bad_request("WEB_FETCH_FAILED", format!("read failed: {e}")))?;
        if buf.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(AppError::bad_request(
                "WEB_FETCH_TOO_LARGE",
                format!("response exceeds cap of {max_bytes} bytes"),
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    let byte_count = buf.len() as u64;
    let html = String::from_utf8_lossy(&buf).into_owned();

    let (title, markdown) = extract_markdown(&html, &final_url);
    let (content, truncated) = truncate_chars(markdown, max_chars);

    Ok(FetchedPage {
        url: url.to_string(),
        final_url,
        title,
        content,
        truncated,
        byte_count,
    })
}

/// Readability extraction → markdown. Best-effort: on extraction failure,
/// convert the raw HTML so the model still gets something usable.
fn extract_markdown(html: &str, url: &str) -> (String, String) {
    use dom_smoothie::Readability;
    match Readability::new(html, Some(url), None).and_then(|mut r| r.parse()) {
        Ok(article) => {
            let title = article.title.clone();
            let md = htmd::convert(&article.content)
                .unwrap_or_else(|_| article.text_content.to_string());
            (title, md)
        }
        Err(_) => (String::new(), htmd::convert(html).unwrap_or_default()),
    }
}

/// Truncate to `max_chars` on a char boundary; returns (text, was_truncated).
fn truncate_chars(mut s: String, max_chars: usize) -> (String, bool) {
    if s.chars().count() <= max_chars {
        return (s, false);
    }
    let end = s
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    s.truncate(end);
    (s, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_char_cap() {
        let (out, trunc) = truncate_chars("hello world".to_string(), 5);
        assert_eq!(out, "hello");
        assert!(trunc);

        let (out, trunc) = truncate_chars("hi".to_string(), 5);
        assert_eq!(out, "hi");
        assert!(!trunc);
    }

    #[test]
    fn truncate_is_char_boundary_safe() {
        // Multi-byte chars must not split mid-codepoint.
        let (out, trunc) = truncate_chars("héllo wörld".to_string(), 4);
        assert_eq!(out.chars().count(), 4);
        assert!(trunc);
    }

    #[tokio::test]
    async fn fetch_rejects_imds_and_private_urls() {
        // The default (non-debug-flag) policy is public-only; IMDS + RFC1918
        // are rejected by the SSRF guard before any network call. These two
        // are blocked under BOTH the default and the DEV_LOCAL test policy, so
        // the assertion holds regardless of WEB_SEARCH_FETCH_ALLOW_LOOPBACK.
        for url in ["http://169.254.169.254/latest/meta-data/", "http://10.0.0.1/"] {
            let err = fetch_url(url, 1_000_000, 10_000, 5).await.unwrap_err();
            assert_eq!(
                err.error_code(),
                "WEB_FETCH_BLOCKED_URL",
                "url {url} must be SSRF-blocked"
            );
        }
    }

    #[test]
    fn extract_markdown_preserves_non_html_content_types() {
        // Non-HTML endpoints (JSON / CSV / XML) won't yield a Readability
        // "article", but the fallback (htmd::convert of the raw body) must
        // still preserve the substantive content rather than dropping it.
        let (_t, json_md) =
            extract_markdown(r#"{"codeword":"VALUE_JSON_123","n":7}"#, "https://api.example.com/x");
        assert!(
            json_md.contains("VALUE_JSON_123"),
            "JSON content must survive extraction; got: {json_md}"
        );

        let (_t, csv_md) =
            extract_markdown("name,score\nalice,VALUE_CSV_456\n", "https://example.com/data.csv");
        assert!(
            csv_md.contains("VALUE_CSV_456"),
            "CSV content must survive extraction; got: {csv_md}"
        );

        let (_t, xml_md) = extract_markdown(
            "<feed><entry><title>VALUE_XML_789</title></entry></feed>",
            "https://example.com/feed.xml",
        );
        assert!(
            xml_md.contains("VALUE_XML_789"),
            "XML content must survive extraction; got: {xml_md}"
        );
    }

    #[test]
    fn extract_markdown_strips_boilerplate_and_keeps_body() {
        let html = r#"<html><head><title>My Article</title></head><body>
            <nav>menu home about</nav>
            <article><h1>Real Heading</h1>
            <p>This is the substantive body paragraph that readability keeps.</p>
            <p>Another meaningful paragraph with enough text to be retained by the extractor.</p>
            </article>
            <footer>copyright junk</footer></body></html>"#;
        let (title, md) = extract_markdown(html, "https://example.com/a");
        assert!(md.contains("Real Heading"), "md was: {md}");
        assert!(md.contains("substantive body"), "md was: {md}");
        // Title best-effort (readability may derive it from <title> or <h1>).
        assert!(!title.is_empty() || md.contains("Real Heading"));
    }
}
