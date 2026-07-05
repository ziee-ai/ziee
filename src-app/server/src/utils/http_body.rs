//! Capped HTTP response-body readers — a shared SSRF/DoS defense.
//!
//! `reqwest` has **no default body-size limit**, so a misbehaving or
//! compromised upstream could stream an unbounded body and exhaust memory.
//! These helpers enforce a hard byte cap (checked against `Content-Length`
//! *and* while streaming, so a lying/absent header can't bypass it).
//!
//! Lifted from `web_search`'s search-provider reader so `web_search` (provider
//! JSON) and `lit_search` (connectors + full-text) share ONE implementation of
//! this security control. NOTE: `web_search/fetch.rs` (page fetch) keeps its own
//! variant — it maps overflow to the `WEB_FETCH_TOO_LARGE` bad-request code,
//! whereas these return `internal_error`. Callers pass their own cap (search
//! JSON is small; literature pages and full-text bodies are larger).

use serde::de::DeserializeOwned;

use crate::common::AppError;
use futures_util::StreamExt;

/// Read a response body into memory with a hard byte cap.
///
/// Rejects early on an oversized `Content-Length`, then re-checks while
/// streaming (a missing/understated header can't smuggle past the cap).
pub async fn read_bytes_capped(
    resp: reqwest::Response,
    max_bytes: u64,
) -> Result<Vec<u8>, AppError> {
    if let Some(len) = resp.content_length()
        && len > max_bytes
    {
        return Err(AppError::internal_error(format!(
            "response too large: {len} bytes (cap {max_bytes})"
        )));
    }
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| AppError::internal_error(format!("response read failed: {e}")))?;
        if buf.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(AppError::internal_error(format!(
                "response exceeds size cap ({max_bytes} bytes)"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Read + deserialize a JSON response with a hard byte cap.
pub async fn read_json_capped<T: DeserializeOwned>(
    resp: reqwest::Response,
    max_bytes: u64,
) -> Result<T, AppError> {
    let bytes = read_bytes_capped(resp, max_bytes).await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::internal_error(format!("response parse failed: {e}")))
}

/// Read a response body as a (lossy) UTF-8 string with a hard byte cap — for
/// the XML connectors (arXiv Atom, PubMed efetch).
pub async fn read_text_capped(
    resp: reqwest::Response,
    max_bytes: u64,
) -> Result<String, AppError> {
    let bytes = read_bytes_capped(resp, max_bytes).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response with a KNOWN Content-Length (built from an in-memory body).
    /// reqwest derives `content_length()` from the body, exercising the early
    /// Content-Length rejection branch.
    fn sized_response(body: Vec<u8>) -> reqwest::Response {
        let http_resp = http::Response::builder()
            .status(200)
            .body(body)
            .unwrap();
        reqwest::Response::from(http_resp)
    }

    /// A response with UNKNOWN length (a streaming body), so `content_length()`
    /// is None and the cap can only be enforced while streaming.
    fn streaming_response(chunks: Vec<Vec<u8>>) -> reqwest::Response {
        let stream = futures_util::stream::iter(
            chunks
                .into_iter()
                .map(|c| Ok::<Vec<u8>, std::io::Error>(c)),
        );
        let body = reqwest::Body::wrap_stream(stream);
        let http_resp = http::Response::builder().status(200).body(body).unwrap();
        reqwest::Response::from(http_resp)
    }

    #[tokio::test]
    async fn passes_body_under_cap() {
        let resp = sized_response(b"hello world".to_vec());
        let out = read_bytes_capped(resp, 1024).await.unwrap();
        assert_eq!(out, b"hello world");
    }

    #[tokio::test]
    async fn rejects_oversized_content_length_before_reading() {
        // Body length (20) is known and exceeds the cap (8) → early reject.
        let resp = sized_response(vec![0u8; 20]);
        let err = read_bytes_capped(resp, 8).await.unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[tokio::test]
    async fn enforces_cap_while_streaming_when_length_unknown() {
        // No Content-Length; the stream yields 30 bytes total against a 16-byte
        // cap. The streaming guard (not the header check) must trip.
        let resp = streaming_response(vec![vec![0u8; 10], vec![0u8; 10], vec![0u8; 10]]);
        assert!(resp.content_length().is_none());
        let err = read_bytes_capped(resp, 16).await.unwrap_err();
        assert!(err.to_string().contains("size cap"));
    }

    #[tokio::test]
    async fn read_json_capped_parses_within_cap_and_errors_over_cap() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Payload {
            n: u32,
        }
        let ok = sized_response(br#"{"n":7}"#.to_vec());
        let parsed: Payload = read_json_capped(ok, 1024).await.unwrap();
        assert_eq!(parsed, Payload { n: 7 });

        // Same JSON but a cap smaller than the body → rejected before parse.
        let over = sized_response(br#"{"n":7}"#.to_vec());
        assert!(read_json_capped::<Payload>(over, 3).await.is_err());
    }

    #[tokio::test]
    async fn read_text_capped_is_lossy_utf8() {
        // Invalid UTF-8 bytes must not error — they're replaced.
        let resp = sized_response(vec![0xff, 0xfe, b'h', b'i']);
        let out = read_text_capped(resp, 1024).await.unwrap();
        assert!(out.ends_with("hi"));
    }
}
