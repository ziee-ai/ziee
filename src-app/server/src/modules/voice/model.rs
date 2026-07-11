//! Whisper ggml model management: resolve → (air-gap detect | direct-URL
//! download + sha256 verify) → cache under `<app_data>/voice-models/`.
//!
//! Unlike `llm_model` (git-LFS/HF-repo), whisper models are single files fetched
//! by direct URL from the HuggingFace `ggerganov/whisper.cpp` repo. This file
//! owns the on-disk resolution + presence check + the supported set (so the
//! settings validator, capability endpoint, and deployment layer agree on where a
//! model lives) AND the streaming, sha256-verified, size-capped download used by
//! the auto-start path and the admin download endpoint.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::common::AppError;

/// Pinned sha256 of each downloadable `ggml-<name>.bin`, hex lowercase.
///
/// A downloaded file whose digest does not match its pinned entry is deleted and
/// the download fails — the model bytes are never trusted from the network
/// alone. A model with NO entry here (or an all-zero placeholder) is likewise
/// rejected (fail-closed): a supported model must carry a real pin before it can
/// be installed from the network.
///
// Real digests: the git-LFS `oid sha256` of each `ggml-<name>.bin` from
// https://huggingface.co/ggerganov/whisper.cpp (the LFS oid IS the file's
// sha256). Fetched from the HF raw LFS pointers. A downloaded file that does not
// match its pinned digest is deleted and the download fails.
pub const KNOWN_MODEL_SHA256: &[(&str, &str)] = &[
    (
        "tiny",
        "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    ),
    (
        "base",
        "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    ),
    (
        "base.en",
        "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    ),
    (
        "small",
        "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    ),
];

/// Hard cap on a downloaded model file. The largest whisper model (`large-v3`) is
/// ~3.1 GB; 5 GiB leaves headroom for future/quantized variants while bounding a
/// malicious/mis-sized response. Whisper model files are upstream-bounded, so this
/// is a safety ceiling, not a per-deployment tunable (DEC-6).
pub const MAX_MODEL_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Cap on an admin-uploaded model file (same bound + rationale as [`MAX_MODEL_BYTES`]).
pub const VOICE_MODEL_MAX_UPLOAD_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Validate that `bytes` begin with a whisper ggml (`ggml`) or GGUF (`GGUF`) magic.
/// Uploaded / arbitrary-URL model files are checked so a non-model blob is rejected
/// before it lands in the library.
pub fn has_whisper_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && (&bytes[..4] == b"ggml" || &bytes[..4] == b"GGUF")
}

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

/// The default on-disk path for a model (`ggml-<name>.bin`), present or not.
pub fn model_path(name: &str) -> PathBuf {
    models_dir().join(model_filename(name))
}

/// Resolve the actual installed file for `name`, checking BOTH the `.bin` and
/// `.gguf` variants (an uploaded/downloaded GGUF is stored `ggml-<name>.gguf`).
/// Returns the first non-empty file that exists. This is the runtime's source of
/// truth for "which file to serve", so a library model (catalog/url/upload) with
/// any supported name — not just the 4 built-in defaults — actually runs.
pub fn installed_model_path(name: &str) -> Option<PathBuf> {
    for fname in [format!("ggml-{name}.bin"), format!("ggml-{name}.gguf")] {
        let p = models_dir().join(&fname);
        if std::fs::metadata(&p).map(|m| m.is_file() && m.len() > 0).unwrap_or(false) {
            return Some(p);
        }
    }
    None
}

/// True when a non-empty model file exists on disk (downloaded or pre-staged).
pub fn model_present(name: &str) -> bool {
    installed_model_path(name).is_some()
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

async fn ensure_model_with_progress<F>(name: &str, cb: F) -> Result<PathBuf, AppError>
where
    F: Fn(u64, Option<u64>) + Send + Sync,
{
    // Already installed (any library model — catalog/url/upload, .bin or .gguf) →
    // serve it directly, regardless of whether it's one of the 4 built-in
    // auto-downloadable defaults. This is what lets an activated `large-v3` run.
    if let Some(existing) = installed_model_path(name) {
        let len = std::fs::metadata(&existing).map(|m| m.len()).unwrap_or(0);
        cb(len, Some(len));
        return Ok(existing);
    }

    // Absent → we can only AUTO-download a known built-in default (pinned URL);
    // an arbitrary library model that isn't on disk can't be re-fetched here.
    if !is_supported_model(name) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("whisper model {name:?} is not installed (download or upload it first)"),
        ));
    }

    let dest = model_path(name);

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

    // Stream to a temp file, hashing as we go, with a hard byte cap. The file
    // writes go through `tokio::fs` so a multi-hundred-MB download never blocks
    // the executor thread (this can run under the auto-start START_LOCK).
    // Per-attempt unique temp name: the admin download endpoint and a
    // transcribe-triggered auto-start can both fetch the same absent model
    // concurrently (they don't share a lock), and a shared `<name>.tmp` would
    // interleave their byte streams into a spurious sha256 mismatch. A uuid
    // suffix isolates each attempt; the loser's temp is cleaned up on drop/error.
    let tmp = dir.join(format!("{}.{}.tmp", model_filename(name), uuid::Uuid::new_v4()));
    let mut file = tokio::fs::File::create(&tmp)
        .await
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
                .await
                .map_err(|e| AppError::internal_error(format!("write model chunk: {e}")))?;
            cb(downloaded, total);
        }
        file.flush()
            .await
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

    // Verify sha256 against the pinned table. Fail CLOSED: a model with no real
    // pin (missing entry or an all-zero placeholder) is rejected rather than
    // installed unverified — we never trust network bytes for a supported model
    // without a real digest. All shipped models have real pins, so this only
    // hardens the future add-a-model path.
    let actual = hex_lower(&hasher.finalize());
    match known_sha256(name) {
        Some(expected) if expected.bytes().all(|b| b == b'0') => {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::internal_error(format!(
                "voice: model {name} has a placeholder sha256 pin; refusing to install \
                 unverified bytes (pin the real digest before enabling this model)"
            )));
        }
        Some(expected) => {
            if !expected.eq_ignore_ascii_case(&actual) {
                let _ = std::fs::remove_file(&tmp);
                return Err(AppError::internal_error(format!(
                    "sha256 mismatch for whisper model {name}: expected {expected}, got {actual}"
                )));
            }
        }
        None => {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::internal_error(format!(
                "voice: model {name} has no pinned sha256; refusing to install unverified bytes"
            )));
        }
    }

    // Atomically publish.
    finalize_download(&tmp, &dest)?;
    tracing::info!("voice: whisper model {name} ready ({downloaded} bytes)");
    Ok(dest)
}

// =====================================================================
// Unified model-library download (catalog / HF-repo / arbitrary URL)
// =====================================================================

/// Where a model download's bytes come from + how to verify them.
pub struct ModelDownloadSpec {
    /// Stored short name (also the `settings.model` pointer value).
    pub name: String,
    /// On-disk filename (`ggml-<name>.bin` for catalog; else the source filename).
    pub filename: String,
    /// The resolved https URL to stream from.
    pub url: String,
    /// The HF-advertised git-LFS oid (sha256) to verify against. `Some` → the
    /// download is fail-closed on mismatch and stored `verified=true`; `None` →
    /// sha256 is only computed and stored `verified=false`.
    pub expected_sha256: Option<String>,
    /// SSRF-validate the URL before fetching. True for user-supplied arbitrary
    /// URLs; false for the admin-configured (trusted) catalog/HF source.
    pub ssrf_check: bool,
}

/// The result of a completed model-library download.
pub struct DownloadedModel {
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub verified: bool,
}

/// Stream a model file into `voice-models/`, reporting `(received, total)`
/// progress via `cb` and cooperatively cancelling when `cancelled` is set (or the
/// caller's shutdown race fires). Validates the whisper magic, enforces the size
/// cap, verifies against `expected_sha256` when present (fail-closed), computes
/// the digest otherwise, and atomically publishes. Cleans up the temp file on any
/// error/cancel (fixes the shutdown temp-leak of the legacy path).
pub async fn download_model_file<F>(
    spec: &ModelDownloadSpec,
    cb: F,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<DownloadedModel, AppError>
where
    F: Fn(u64, Option<u64>) + Send + Sync,
{
    use std::sync::atomic::Ordering;

    if spec.ssrf_check {
        crate::utils::url_validator::validate_outbound_url(
            &spec.url,
            &crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
        )
        .map_err(|e| {
            AppError::bad_request(
                "VOICE_MODEL_URL_REJECTED",
                format!("model URL rejected by SSRF policy: {e}"),
            )
        })?;
    }

    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(format!("create voice-models dir: {e}")))?;

    // For user-supplied (arbitrary) URLs, use the SSRF-guarding client: it pins a
    // DNS resolver that rejects private/loopback/IMDS targets AND re-validates
    // every redirect hop (a plain client would follow a 3xx from a public URL to
    // loopback — the SSRF-via-redirect bypass). The trusted catalog/HF source uses
    // a plain no-proxy client.
    let client = if spec.ssrf_check {
        crate::utils::url_validator::validated_client_builder(
            crate::utils::url_validator::OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS,
        )
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .map_err(AppError::internal_with_id)?
    } else {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(1800))
            .no_proxy()
            .build()
            .map_err(AppError::internal_with_id)?
    };
    let redacted = redact_url(&spec.url);
    let resp = client
        .get(&spec.url)
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("download request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "model download returned HTTP {} for {redacted}",
            resp.status(),
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

    let tmp = dir.join(format!("{}.{}.tmp", spec.filename, uuid::Uuid::new_v4()));
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| AppError::internal_error(format!("create temp model file: {e}")))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut head: Vec<u8> = Vec::with_capacity(4);
    let mut stream = resp.bytes_stream();

    let result: Result<(), AppError> = async {
        while let Some(chunk) = stream.next().await {
            if cancelled.load(Ordering::Relaxed) {
                return Err(AppError::bad_request(
                    "VOICE_MODEL_DOWNLOAD_CANCELLED",
                    "download cancelled",
                ));
            }
            let chunk =
                chunk.map_err(|e| AppError::internal_error(format!("download read failed: {e}")))?;
            if head.len() < 4 {
                head.extend_from_slice(&chunk[..chunk.len().min(4 - head.len())]);
                if head.len() >= 4 && !has_whisper_magic(&head) {
                    return Err(AppError::bad_request(
                        "VOICE_MODEL_INVALID",
                        "file is not a whisper ggml/GGUF model (bad magic)",
                    ));
                }
            }
            downloaded += chunk.len() as u64;
            if downloaded > MAX_MODEL_BYTES {
                return Err(AppError::bad_request(
                    "VOICE_MODEL_TOO_LARGE",
                    format!("model exceeds cap of {MAX_MODEL_BYTES} bytes"),
                ));
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|e| AppError::internal_error(format!("write model chunk: {e}")))?;
            cb(downloaded, total);
        }
        file.flush()
            .await
            .map_err(|e| AppError::internal_error(format!("flush model file: {e}")))?;
        Ok(())
    }
    .await;

    // Always clean up the temp file on any failure/cancel (no leak).
    if let Err(e) = result {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(file);

    if downloaded == 0 || !has_whisper_magic(&head) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::bad_request(
            "VOICE_MODEL_INVALID",
            "download produced no valid whisper model bytes",
        ));
    }

    let actual = hex_lower(&hasher.finalize());
    let verified = match &spec.expected_sha256 {
        Some(expected) => {
            if !expected.eq_ignore_ascii_case(&actual) {
                let _ = std::fs::remove_file(&tmp);
                return Err(AppError::bad_request(
                    "VOICE_MODEL_SHA_MISMATCH",
                    format!(
                        "sha256 mismatch for {}: expected {expected}, got {actual}",
                        spec.name
                    ),
                ));
            }
            true
        }
        None => false,
    };

    let dest = dir.join(&spec.filename);
    finalize_download(&tmp, &dest)?;
    Ok(DownloadedModel {
        filename: spec.filename.clone(),
        size_bytes: downloaded,
        sha256: actual,
        verified,
    })
}

/// A streamed-to-disk upload awaiting validation + finalization.
pub struct UploadTemp {
    pub tmp: PathBuf,
    pub size: u64,
    pub sha256: String,
    /// First up-to-4 bytes, for the caller's magic check.
    pub head: Vec<u8>,
}

/// Stream a multipart upload field to a temp file under `voice-models/` — hashing
/// + capturing the head + enforcing the size cap AS IT ARRIVES (never buffering
/// the whole multi-GB file in RAM). The caller validates `head`/name then calls
/// [`finalize_upload_temp`]; on any early return the temp is cleaned up.
pub async fn stream_upload_to_temp(
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<UploadTemp, AppError> {
    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(format!("create voice-models dir: {e}")))?;
    let tmp = dir.join(format!(".upload-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| AppError::internal_error(format!("create temp model file: {e}")))?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;
    let mut head: Vec<u8> = Vec::with_capacity(4);

    let res: Result<(), AppError> = async {
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| AppError::bad_request("UPLOAD_ERROR", format!("read upload: {e}")))?
        {
            size += chunk.len() as u64;
            if size > VOICE_MODEL_MAX_UPLOAD_BYTES {
                return Err(AppError::bad_request(
                    "VOICE_MODEL_TOO_LARGE",
                    format!("upload exceeds cap of {VOICE_MODEL_MAX_UPLOAD_BYTES} bytes"),
                ));
            }
            if head.len() < 4 {
                head.extend_from_slice(&chunk[..chunk.len().min(4 - head.len())]);
            }
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|e| AppError::internal_error(format!("write upload: {e}")))?;
        }
        file.flush()
            .await
            .map_err(|e| AppError::internal_error(format!("flush upload: {e}")))?;
        Ok(())
    }
    .await;

    if let Err(e) = res {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(file);
    Ok(UploadTemp {
        tmp,
        size,
        sha256: hex_lower(&hasher.finalize()),
        head,
    })
}

/// Atomically move a validated upload temp into place as `filename`.
pub fn finalize_upload_temp(tmp: &Path, filename: &str) -> Result<(), AppError> {
    finalize_download(tmp, &models_dir().join(filename))
}

/// Delete an upload temp (validation failed).
pub fn discard_temp(tmp: &Path) {
    let _ = std::fs::remove_file(tmp);
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

/// Strip any `user:pass@` userinfo from a URL before it lands in a log line or an
/// admin-visible error (an arbitrary-URL download could embed credentials).
fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            if !u.username().is_empty() || u.password().is_some() {
                let _ = u.set_username("");
                let _ = u.set_password(None);
            }
            u.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
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
