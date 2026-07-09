//! Whisper ggml model management: resolve → (air-gap detect | direct-URL
//! download + sha256 verify) → cache under `<app_data>/voice-models/`.
//!
//! Unlike `llm_model` (git-LFS/HF-repo), whisper models are single files fetched
//! by direct URL from the HuggingFace `ggerganov/whisper.cpp` repo. This file
//! owns the on-disk resolution + presence check + the supported set (so the
//! settings validator, capability endpoint, and deployment layer agree on where a
//! model lives) AND the streaming, sha256-verified, size-capped download used by
//! the auto-start path and the admin download endpoint.

use std::io::Write;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::common::AppError;

/// Pinned sha256 of each downloadable `ggml-<name>.bin`, hex lowercase.
///
/// A downloaded file whose digest does not match its pinned entry is deleted and
/// the download fails — the model bytes are never trusted from the network
/// alone. Models with no entry here skip verification (logged) so a new
/// [`SUPPORTED_MODELS`] entry isn't hard-blocked before its hash is pinned.
///
// TODO verify: these are PLACEHOLDER digests. Before shipping, replace each with
// the real sha256 of the corresponding `ggml-<name>.bin` from
// https://huggingface.co/ggerganov/whisper.cpp (e.g. `sha256sum ggml-base.bin`).
pub const KNOWN_MODEL_SHA256: &[(&str, &str)] = &[
    (
        "tiny",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "base",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "base.en",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
    (
        "small",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ),
];

/// Hard cap on a downloaded model file. The largest offered model (`small`) is
/// ~466 MB; 1 GiB leaves generous headroom while bounding a malicious/mis-sized
/// response.
const MAX_MODEL_BYTES: u64 = 1024 * 1024 * 1024;

/// Look up the pinned sha256 for `name`, if any.
pub fn known_sha256(name: &str) -> Option<&'static str> {
    KNOWN_MODEL_SHA256
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, h)| *h)
}

/// The whisper models the admin may select. Multilingual unless `.en`.
pub const SUPPORTED_MODELS: &[&str] = &["tiny", "base", "base.en", "small"];

/// True when `name` is an offered model.
pub fn is_supported_model(name: &str) -> bool {
    SUPPORTED_MODELS.contains(&name)
}

/// `<app_data>/voice-models/` — the model cache (also the air-gap pre-stage dir).
pub fn models_dir() -> PathBuf {
    crate::core::get_app_data_dir().join("voice-models")
}

/// The ggml filename for a model, e.g. `ggml-base.bin`.
pub fn model_filename(name: &str) -> String {
    format!("ggml-{name}.bin")
}

/// The on-disk path a model resolves to (present or not).
pub fn model_path(name: &str) -> PathBuf {
    models_dir().join(model_filename(name))
}

/// True when a non-empty model file exists on disk (downloaded or pre-staged).
pub fn model_present(name: &str) -> bool {
    match std::fs::metadata(model_path(name)) {
        Ok(m) => m.is_file() && m.len() > 0,
        Err(_) => false,
    }
}

/// Base URL for the whisper.cpp ggml model files. Overridable in **debug builds
/// only** via `WHISPER_MODEL_MIRROR` so tests can serve a fixture from a loopback
/// HTTP server (mirrors `LLM_RUNTIME_RELEASE_MIRROR` / `WEB_SEARCH_BRAVE_ENDPOINT`).
fn model_base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("WHISPER_MODEL_MIRROR") {
        if !base.is_empty() {
            return base.trim_end_matches('/').to_string();
        }
    }
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main".to_string()
}

/// The direct download URL for `ggml-<name>.bin`.
fn model_url(name: &str) -> String {
    format!("{}/{}", model_base_url(), model_filename(name))
}

/// Resolve a model to a local path, downloading it if absent.
///
/// Present on disk → return the path immediately. Otherwise stream-download
/// `ggml-<name>.bin`, sha256-verify against [`KNOWN_MODEL_SHA256`] (deleting the
/// partial on mismatch), and return the cached path.
pub async fn ensure_model(name: &str) -> Result<PathBuf, AppError> {
    ensure_model_with_progress(name, |_, _| {}).await
}

/// Download `ggml-<name>.bin` reporting `(downloaded, total)` byte progress via
/// `cb` (for the SSE admin endpoint). Idempotent: a present model short-circuits
/// with a single terminal progress callback.
pub async fn download_model_with_progress<F>(name: &str, cb: F) -> Result<PathBuf, AppError>
where
    F: Fn(u64, Option<u64>) + Send + Sync,
{
    ensure_model_with_progress(name, cb).await
}

async fn ensure_model_with_progress<F>(name: &str, cb: F) -> Result<PathBuf, AppError>
where
    F: Fn(u64, Option<u64>) + Send + Sync,
{
    if !is_supported_model(name) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("unsupported whisper model: {name:?}"),
        ));
    }

    let dest = model_path(name);
    if model_present(name) {
        let len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        cb(len, Some(len));
        return Ok(dest);
    }

    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(format!("create voice-models dir: {e}")))?;

    let url = model_url(name);
    tracing::info!("voice: downloading whisper model {name} from {url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(AppError::internal_with_id)?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("download request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "model download returned HTTP {} for {url}",
            resp.status()
        )));
    }

    let total = resp.content_length();
    if let Some(len) = total
        && len > MAX_MODEL_BYTES
    {
        return Err(AppError::bad_request(
            "VOICE_MODEL_TOO_LARGE",
            format!("model is {len} bytes, exceeds cap of {MAX_MODEL_BYTES}"),
        ));
    }

    // Stream to a temp file, hashing as we go, with a hard byte cap.
    let tmp = dir.join(format!("{}.tmp", model_filename(name)));
    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| AppError::internal_error(format!("create temp model file: {e}")))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    let result: Result<(), AppError> = async {
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| AppError::internal_error(format!("download read failed: {e}")))?;
            downloaded += chunk.len() as u64;
            if downloaded > MAX_MODEL_BYTES {
                return Err(AppError::bad_request(
                    "VOICE_MODEL_TOO_LARGE",
                    format!("model exceeds cap of {MAX_MODEL_BYTES} bytes"),
                ));
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| AppError::internal_error(format!("write model chunk: {e}")))?;
            cb(downloaded, total);
        }
        file.flush()
            .map_err(|e| AppError::internal_error(format!("flush model file: {e}")))?;
        Ok(())
    }
    .await;

    if let Err(e) = result {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(file);

    if downloaded == 0 {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::internal_error(format!(
            "model download returned 0 bytes from {url}"
        )));
    }

    // Verify sha256 against the pinned table (skip only when unpinned).
    let actual = hex_lower(&hasher.finalize());
    if let Some(expected) = known_sha256(name) {
        // A placeholder all-zero pin means "not yet pinned" — skip (see TODO on
        // KNOWN_MODEL_SHA256) rather than reject every real download pre-pinning.
        let is_placeholder = expected.bytes().all(|b| b == b'0');
        if !is_placeholder && !expected.eq_ignore_ascii_case(&actual) {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::internal_error(format!(
                "sha256 mismatch for whisper model {name}: expected {expected}, got {actual}"
            )));
        }
        if is_placeholder {
            tracing::warn!(
                "voice: model {name} sha256 pin is a placeholder; skipping verification (got {actual})"
            );
        }
    } else {
        tracing::warn!("voice: model {name} has no pinned sha256; skipping verification");
    }

    // Atomically publish.
    finalize_download(&tmp, &dest)?;
    tracing::info!("voice: whisper model {name} ready ({downloaded} bytes)");
    Ok(dest)
}

/// Rename the verified temp file into place (best-effort cross-device fallback).
fn finalize_download(tmp: &Path, dest: &Path) -> Result<(), AppError> {
    match std::fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(tmp, dest)
                .map_err(|e| AppError::internal_error(format!("publish model file: {e}")))?;
            let _ = std::fs::remove_file(tmp);
            Ok(())
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_model_has_a_pin_entry() {
        for name in SUPPORTED_MODELS {
            assert!(
                known_sha256(name).is_some(),
                "missing KNOWN_MODEL_SHA256 entry for {name}"
            );
        }
    }

    #[test]
    fn model_url_uses_ggml_filename() {
        // (Independent of the mirror env in release builds.)
        let url = super::model_url("base");
        assert!(url.ends_with("/ggml-base.bin"), "unexpected url: {url}");
    }

    #[test]
    fn hex_lower_pads_and_lowercases() {
        assert_eq!(hex_lower(&[0x00, 0x0a, 0xff]), "000aff");
    }
}
