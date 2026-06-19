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
