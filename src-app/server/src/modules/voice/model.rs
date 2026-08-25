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

/// whisper.cpp's legacy ggml container magic, as declared upstream
/// (`GGML_FILE_MAGIC`). It is written to disk as a **native-endian `u32`**, NOT as
/// an ASCII string — so on every little-endian host (x86_64, aarch64: i.e. every
/// platform ziee ships) a real `ggml-*.bin` begins with the bytes
/// `6c 6d 67 67`, which read as ASCII `lmgg` — the REVERSE of `ggml`.
///
/// This is the whole of the "bad magic" defect: the check used to compare against
/// the ASCII spelling `b"ggml"`, which no real whisper.cpp model file has ever
/// begun with, so every catalog / URL / upload install of a genuine model was
/// rejected on its first chunk. See `.lifecycle/voice-model-bad-magic/`.
pub const GGML_FILE_MAGIC: u32 = 0x6767_6d6c;

/// The on-disk byte order of [`GGML_FILE_MAGIC`] on a little-endian host —
/// `6c 6d 67 67` (`lmgg`). This is what real whisper.cpp model files start with.
pub const GGML_MAGIC_LE: [u8; 4] = GGML_FILE_MAGIC.to_le_bytes();

/// The big-endian ordering of [`GGML_FILE_MAGIC`] — `67 67 6d 6c` (ASCII `ggml`).
/// Accepted defensively for a file authored on a big-endian host; this is also the
/// (only) form the pre-fix check accepted, so keeping it makes the corrected check
/// a pure WIDENING of the previous accept-set — no input that used to pass can now
/// fail.
pub const GGML_MAGIC_BE: [u8; 4] = GGML_FILE_MAGIC.to_be_bytes();

/// The GGUF container magic. Unlike ggml's, the GGUF spec stores this as the
/// literal ASCII bytes `GGUF`, so no byte-order handling is needed — matching
/// `llm_local_runtime::engine::metadata`'s check.
pub const GGUF_MAGIC: [u8; 4] = *b"GGUF";

/// Validate that `bytes` begin with a whisper ggml or GGUF container magic.
/// Downloaded / uploaded / arbitrary-URL model files are checked so a non-model
/// blob (an HTML error page, a zip, a truncated body) is rejected before it lands
/// in the library.
///
/// Accepts [`GGML_MAGIC_LE`] (what real files are), [`GGML_MAGIC_BE`], and
/// [`GGUF_MAGIC`]. Fewer than 4 bytes cannot identify a container and is rejected;
/// callers distinguish that case for the user via [`ModelRejection`].
pub fn has_whisper_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && matches!(
            <[u8; 4]>::try_from(&bytes[..4]),
            Ok(GGML_MAGIC_LE) | Ok(GGML_MAGIC_BE) | Ok(GGUF_MAGIC)
        )
}

/// Why a candidate model file was refused. Each variant is a genuinely different
/// user situation and gets its own error code + message — the pre-fix code folded
/// "the response body was empty" into the magic error, so an empty HTTP 200 was
/// reported to the user as "bad magic", which is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRejection {
    /// Zero bytes arrived — an empty response body / an empty uploaded file.
    Empty,
    /// Fewer than 4 bytes arrived: too short to identify any container.
    Truncated,
    /// Bytes arrived, but they are not a whisper ggml/GGUF container.
    BadMagic,
}

impl ModelRejection {
    /// Classify an observed file head. `total_len` is the full byte count (which
    /// may exceed `head.len()`, since callers only retain the first few bytes).
    pub fn classify(head: &[u8], total_len: u64) -> Option<Self> {
        if total_len == 0 {
            return Some(ModelRejection::Empty);
        }
        if head.len() < 4 {
            return Some(ModelRejection::Truncated);
        }
        if !has_whisper_magic(head) {
            return Some(ModelRejection::BadMagic);
        }
        None
    }

    /// The stable error code for this rejection.
    pub fn code(self) -> &'static str {
        match self {
            ModelRejection::Empty => "VOICE_MODEL_EMPTY_DOWNLOAD",
            ModelRejection::Truncated => "VOICE_MODEL_TRUNCATED",
            ModelRejection::BadMagic => "VOICE_MODEL_INVALID",
        }
    }

    /// An actionable message: what was FOUND, what was EXPECTED, and the
    /// CORRECTIVE ACTION. `source` names the thing being rejected for the user
    /// (e.g. `"the downloaded file"` / `"the uploaded file"`).
    pub fn message(self, source: &str, head: &[u8]) -> String {
        let expected = "a whisper model file (a `ggml` or `GGUF` container)";
        match self {
            ModelRejection::Empty => format!(
                "{source} is empty (0 bytes). Expected {expected}. \
                 The source returned no data — check that the URL points directly at the \
                 model file (not a web page or a redirect), then try the download again."
            ),
            ModelRejection::Truncated => format!(
                "{source} ended after {} byte(s) — too short to be a model. Expected {expected}. \
                 The transfer was cut short; try the download again, and if it keeps failing \
                 check your connection to the source.",
                head.len()
            ),
            ModelRejection::BadMagic => format!(
                "{source} is not a whisper model: it starts with {} instead of a recognised \
                 container header. Expected {expected}. \
                 This usually means the URL served a web page or an error message rather than \
                 the model itself — check that it points directly at the raw `.bin`/`.gguf` \
                 file, then re-download. If the file is already installed, remove it and \
                 install it again.",
                describe_head(head)
            ),
        }
    }

    /// Build the `AppError` for this rejection.
    pub fn to_error(self, source: &str, head: &[u8]) -> AppError {
        AppError::bad_request(self.code(), self.message(source, head))
    }
}

/// Render an observed file head as hex plus its printable ASCII, so a user (or a
/// log reader) can tell at a glance that they got an HTML page rather than a
/// model — e.g. ``​`3c 21 44 4f` ("<!DO")``.
pub fn describe_head(head: &[u8]) -> String {
    let shown = &head[..head.len().min(4)];
    if shown.is_empty() {
        return "no data".to_string();
    }
    let hex = shown.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
    let printable: String = shown
        .iter()
        .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' })
        .collect();
    format!("`{hex}` (\"{printable}\")")
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
                    // Fail fast on the very first bytes rather than streaming a
                    // whole HTML error page to disk.
                    return Err(ModelRejection::BadMagic.to_error("the downloaded file", &head));
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

    // Re-check after the stream ends. An empty body, a body shorter than the
    // 4-byte header, and a body with the wrong magic are three DIFFERENT user
    // situations and must not be collapsed into one message (the pre-fix code
    // reported an empty HTTP 200 as "bad magic", which is false).
    if let Some(rejection) = ModelRejection::classify(&head, downloaded) {
        let _ = std::fs::remove_file(&tmp);
        return Err(rejection.to_error("the downloaded file", &head));
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
///
/// Publishing is the LAST failure exit of an acquisition, and it is the one that
/// can leave a **partial destination** behind: `std::fs::copy` that dies part-way
/// (ENOSPC, EIO) leaves a truncated `ggml-<name>.bin`, which
/// [`installed_model_path`] — an exists + non-empty check — would then report as
/// an installed model, and the runtime would try to load it. That is the
/// "a failed acquisition left a broken artifact behind" class this branch exists
/// to close (INV-3), so BOTH sides are removed before the error propagates.
fn finalize_download(tmp: &Path, dest: &Path) -> Result<(), AppError> {
    match std::fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) => match std::fs::copy(tmp, dest) {
            Ok(_) => {
                let _ = std::fs::remove_file(tmp);
                Ok(())
            }
            Err(e) => {
                // Never leave a partial `dest` that would read as installed,
                // and never leak the temp.
                let _ = std::fs::remove_file(dest);
                let _ = std::fs::remove_file(tmp);
                Err(AppError::internal_error(format!("publish model file: {e}")))
            }
        },
    }
}

/// A `*.tmp` under `voice-models/` is only reclaimed by its own writer's error
/// path. A SIGKILL / OOM-kill / power loss mid-download (or mid-upload) leaves
/// one behind forever — up to 5 GiB of dead bytes per orphan, since the cap is
/// enforced as they arrive. Nothing else ever deletes them: the library list
/// comes from the DB, and [`installed_model_path`] only looks at
/// `ggml-<name>.{bin,gguf}`, so an orphan is invisible as well as permanent.
///
/// Swept at module init (see `voice::VoiceModule::init`). `min_age` guards the
/// case of another process sharing the data dir with a download genuinely in
/// flight — a `.tmp` younger than that is left alone.
pub const STALE_TEMP_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// Remove `*.tmp` files under `dir` last modified more than `min_age` ago.
/// Returns how many were reclaimed. Never fails the caller: an unreadable dir
/// (not created yet on a fresh install) or an undeletable entry is a no-op.
pub fn sweep_stale_temps(dir: &Path, min_age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tmp") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let old_enough = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age >= min_age);
        if old_enough && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
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

    /// The first four bytes of a REAL `ggml-base.bin` /`ggml-base-q5_1.bin` /
    /// `ggml-base-q8_0.bin` fetched from `ggerganov/whisper.cpp`, transcribed
    /// from an actual `curl … | xxd` of the published files:
    ///
    /// ```text
    /// 00000000: 6c6d 6767 99ca 0000 dc05 0000 0002 0000  lmgg............
    /// ```
    ///
    /// This literal is the ANCHOR of the whole fix. It is deliberately written
    /// out by hand from an observed file rather than derived from
    /// `GGML_FILE_MAGIC`/`has_whisper_magic`, so it stays true even if someone
    /// "fixes" the constant back to the wrong byte order — the mistake that
    /// shipped. Evidence: `.lifecycle/voice-model-bad-magic/BUG_ANALYSIS.md` E3.
    const REAL_WHISPER_GGML_FILE_HEAD: [u8; 4] = [0x6c, 0x6d, 0x67, 0x67];

    // TEST-8 [acceptance][INV-7] — the real on-disk format is accepted. Written
    // against the FORMAT (an observed file's bytes), never against the
    // implementation's own definition of the magic, so a byte-order regression
    // turns this red even if every other fixture were rewritten to match it.
    #[test]
    fn accepts_the_real_on_disk_whisper_ggml_magic() {
        assert!(
            has_whisper_magic(&REAL_WHISPER_GGML_FILE_HEAD),
            "a REAL whisper.cpp ggml file (head {:02x?}) must be accepted — this is the \
             exact defect that made every model install fail",
            REAL_WHISPER_GGML_FILE_HEAD
        );
        // The constant and the observed file must agree. If this fails, the
        // constant's byte order is wrong (or upstream changed the format).
        assert_eq!(
            GGML_MAGIC_LE, REAL_WHISPER_GGML_FILE_HEAD,
            "GGML_FILE_MAGIC's little-endian serialization must equal the bytes a real \
             whisper.cpp model file begins with"
        );
        // And the ASCII spelling `ggml` is NOT what a real file starts with —
        // pinning the distinction the original code got backwards.
        assert_ne!(
            REAL_WHISPER_GGML_FILE_HEAD, *b"ggml",
            "a real ggml file does NOT begin with the ASCII bytes `ggml`"
        );
    }

    // TEST-1 — the full accept/reject set.
    #[test]
    fn whisper_magic_accepts_ggml_and_gguf_rejects_junk() {
        // The real, little-endian on-disk ordering (`lmgg`).
        assert!(has_whisper_magic(b"lmgg....."));
        assert!(has_whisper_magic(&GGML_MAGIC_LE));
        // The big-endian ordering (ASCII `ggml`) — accepted defensively, and the
        // only form the pre-fix check accepted, so this is a pure widening.
        assert!(has_whisper_magic(b"ggml....."));
        assert!(has_whisper_magic(&GGML_MAGIC_BE));
        // GGUF really is stored as literal ASCII.
        assert!(has_whisper_magic(b"GGUF\x00\x00"));
        // Junk.
        assert!(!has_whisper_magic(b"<htm"));
        assert!(!has_whisper_magic(b"<!DOCTYPE html>"));
        assert!(!has_whisper_magic(b"PK\x03\x04")); // zip
        assert!(!has_whisper_magic(b"lmg")); // too short
        assert!(!has_whisper_magic(b"gg")); // too short
        assert!(!has_whisper_magic(b""));
    }

    // TEST-13 [ITEM-11] — the one product-code invariant from the blast-radius
    // scan: the canonical magic bytes come from ONE named constant. A second
    // hand-written copy is how the fixtures drifted from the format in the first
    // place, so both byte orders must be derived, not re-spelled.
    #[test]
    fn magic_constants_are_derived_from_one_source() {
        assert_eq!(GGML_MAGIC_LE, GGML_FILE_MAGIC.to_le_bytes());
        assert_eq!(GGML_MAGIC_BE, GGML_FILE_MAGIC.to_be_bytes());
        assert_eq!(GGML_MAGIC_BE, *b"ggml", "big-endian form is the ASCII spelling");
        assert_eq!(GGUF_MAGIC, *b"GGUF");
        // The two orderings must be genuinely different, else the test above is
        // vacuous and the original bug would be undetectable.
        assert_ne!(GGML_MAGIC_LE, GGML_MAGIC_BE);
    }

    // TEST-2 — the three rejection conditions are distinct and correctly named.
    #[test]
    fn rejection_classify_distinguishes_empty_truncated_and_bad_magic() {
        // Empty body — must NOT be reported as a magic failure.
        assert_eq!(ModelRejection::classify(&[], 0), Some(ModelRejection::Empty));
        assert_eq!(
            ModelRejection::classify(b"lmgg", 0),
            Some(ModelRejection::Empty),
            "zero total bytes is Empty regardless of any retained head"
        );
        // Fewer than 4 bytes: identifiable as truncated, not as bad magic.
        assert_eq!(ModelRejection::classify(b"lm", 2), Some(ModelRejection::Truncated));
        // Real bytes, wrong container.
        assert_eq!(
            ModelRejection::classify(b"<!DO", 4096),
            Some(ModelRejection::BadMagic)
        );
        // Valid — no rejection.
        assert_eq!(ModelRejection::classify(&GGML_MAGIC_LE, 147_951_465), None);
        assert_eq!(ModelRejection::classify(b"GGUF", 1024), None);

        // Distinct, stable codes.
        assert_eq!(ModelRejection::Empty.code(), "VOICE_MODEL_EMPTY_DOWNLOAD");
        assert_eq!(ModelRejection::Truncated.code(), "VOICE_MODEL_TRUNCATED");
        assert_eq!(ModelRejection::BadMagic.code(), "VOICE_MODEL_INVALID");
        let codes = [
            ModelRejection::Empty.code(),
            ModelRejection::Truncated.code(),
            ModelRejection::BadMagic.code(),
        ];
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), 3, "each condition needs its own code");
    }

    // TEST-5 [acceptance][INV-4] — every rejection message states what was FOUND,
    // what was EXPECTED, and the CORRECTIVE ACTION. This fails if a message
    // regresses to a bare "bad magic" with no found/expected/action content.
    #[test]
    fn rejection_messages_state_found_expected_and_action() {
        let cases = [
            (ModelRejection::Empty, &b""[..]),
            (ModelRejection::Truncated, &b"lm"[..]),
            (ModelRejection::BadMagic, &b"<!DO"[..]),
        ];
        for (rejection, head) in cases {
            let msg = rejection.message("the downloaded file", head);
            let lower = msg.to_lowercase();

            // (b) EXPECTED — always names the container it wanted.
            assert!(
                lower.contains("expected") && lower.contains("ggml") && lower.contains("gguf"),
                "{rejection:?}: message must say what was expected — got: {msg}"
            );
            // (c) ACTION — always tells the user what to do.
            assert!(
                ["try the download again", "re-download", "check that", "remove it"]
                    .iter()
                    .any(|hint| lower.contains(hint)),
                "{rejection:?}: message must state a corrective action — got: {msg}"
            );
            // Names the thing being rejected, so download vs upload is unambiguous.
            assert!(msg.contains("the downloaded file"), "got: {msg}");
            // Never a bare unhelpful phrase.
            assert!(
                msg.len() > 60,
                "{rejection:?}: message is too terse to be actionable — got: {msg}"
            );
        }

        // (a) FOUND — the observed bytes are surfaced for the diagnosable cases.
        let bad = ModelRejection::BadMagic.message("the downloaded file", b"<!DO");
        assert!(
            bad.contains("3c 21 44 4f") && bad.contains("<!DO"),
            "bad-magic message must show the observed bytes (hex + printable) so an \
             HTML error page is self-diagnosing — got: {bad}"
        );
        let empty = ModelRejection::Empty.message("the downloaded file", b"");
        assert!(empty.contains("0 bytes"), "empty message must state the size — got: {empty}");
        let trunc = ModelRejection::Truncated.message("the downloaded file", b"lm");
        assert!(trunc.contains('2'), "truncated message must state how much arrived — got: {trunc}");

        // The upload path reuses the same builder, so wording cannot drift.
        let up = ModelRejection::Empty.message("the uploaded file", b"");
        assert!(up.contains("the uploaded file"));
    }

    #[test]
    fn describe_head_renders_hex_and_printable() {
        assert_eq!(describe_head(b"<!DO"), "`3c 21 44 4f` (\"<!DO\")");
        assert_eq!(describe_head(&GGML_MAGIC_LE), "`6c 6d 67 67` (\"lmgg\")");
        // Non-printable bytes become dots rather than mangling the message.
        assert_eq!(describe_head(&[0x00, 0x01, 0x7f, 0xff]), "`00 01 7f ff` (\"....\")");
        assert_eq!(describe_head(b""), "no data");
        // Never renders more than the 4 magic bytes even if handed more.
        assert_eq!(describe_head(b"GGUF-extra-bytes"), "`47 47 55 46` (\"GGUF\")");
    }

    #[test]
    fn upload_cap_is_a_sane_bound() {
        // 5 GiB — above the largest whisper model (~3.1 GB), below absurd.
        assert_eq!(VOICE_MODEL_MAX_UPLOAD_BYTES, 5 * 1024 * 1024 * 1024);
        assert_eq!(MAX_MODEL_BYTES, VOICE_MODEL_MAX_UPLOAD_BYTES);
    }

    #[test]
    fn installed_path_prefers_bin_then_gguf_naming() {
        // Pure naming contract (no filesystem): the resolver looks for these two.
        assert_eq!(model_filename("large-v3"), "ggml-large-v3.bin");
    }

    // TEST-14 [acceptance][INV-3] — publishing is the last failure exit of an
    // acquisition and the only one that can leave a PARTIAL destination. A
    // truncated `ggml-<name>.bin` would satisfy `installed_model_path`'s
    // exists + non-empty check and be served to the runtime as an installed
    // model — the "a failed acquisition left a broken artifact behind" class.
    #[test]
    fn a_failed_publish_leaves_neither_a_partial_model_nor_a_temp() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("ggml-x.bin.deadbeef.tmp");
        std::fs::write(&tmp, ggml_head_and_filler()).unwrap();
        // Publishing into a directory that does not exist fails BOTH the rename
        // and the copy fallback — the observable stand-in for an ENOSPC/EIO
        // copy, which is not directly inducible in a unit test.
        let dest = dir.path().join("no-such-subdir").join("ggml-x.bin");

        let err = finalize_download(&tmp, &dest).expect_err("publish must fail");
        assert!(
            format!("{err:?}").contains("publish model file"),
            "the failure must preserve context, got: {err:?}"
        );
        assert!(!dest.exists(), "no partial destination may survive a failed publish");
        assert!(!tmp.exists(), "the temp must not leak on a failed publish");

        // …and the success path still moves the file and clears the temp.
        let tmp2 = dir.path().join("ggml-y.bin.cafe.tmp");
        std::fs::write(&tmp2, ggml_head_and_filler()).unwrap();
        let dest2 = dir.path().join("ggml-y.bin");
        finalize_download(&tmp2, &dest2).expect("publish must succeed");
        assert!(dest2.exists() && !tmp2.exists());
    }

    // TEST-15 [acceptance][INV-3] — orphan reclamation. Every failure exit
    // deletes its own temp, but a SIGKILL mid-transfer cannot; nothing else ever
    // would, so without this sweep an orphan is permanent AND invisible.
    #[test]
    fn sweep_reclaims_orphan_temps_and_never_touches_a_model_file() {
        let dir = tempfile::tempdir().unwrap();
        let download_orphan = dir.path().join("ggml-base.bin.0f0f.tmp");
        let upload_orphan = dir.path().join(".upload-1234.tmp");
        let real_model = dir.path().join("ggml-base.bin");
        for p in [&download_orphan, &upload_orphan, &real_model] {
            std::fs::write(p, ggml_head_and_filler()).unwrap();
        }

        // A temp younger than the guard is left alone — a download may be in
        // flight in another process sharing the data dir.
        assert_eq!(
            sweep_stale_temps(dir.path(), STALE_TEMP_MIN_AGE),
            0,
            "a fresh temp must not be reclaimed"
        );
        assert!(download_orphan.exists() && upload_orphan.exists());

        // Past the guard, BOTH shapes of orphan go (the download path's
        // `<filename>.<uuid>.tmp` and the upload path's `.upload-<uuid>.tmp`).
        assert_eq!(sweep_stale_temps(dir.path(), std::time::Duration::ZERO), 2);
        assert!(!download_orphan.exists(), "download temp must be reclaimed");
        assert!(!upload_orphan.exists(), "upload temp must be reclaimed");
        assert!(real_model.exists(), "an installed model file must NEVER be swept");

        // A missing directory (fresh install) is a no-op, not an error.
        assert_eq!(
            sweep_stale_temps(&dir.path().join("nope"), std::time::Duration::ZERO),
            0
        );
    }

    /// Plausible model bytes for the filesystem tests — the REAL on-disk magic
    /// plus filler, so no fixture in this module spells a magic by hand.
    fn ggml_head_and_filler() -> Vec<u8> {
        let mut v = GGML_MAGIC_LE.to_vec();
        v.extend_from_slice(b"filler-bytes");
        v
    }

    // TEST-3: the SSRF boundary the arbitrary-URL download path enforces
    // (`PUBLIC_HTTP_OR_HTTPS`) rejects loopback / IMDS / RFC1918 targets, while a
    // normal public URL is allowed. (The trusted catalog path is not SSRF-checked.)
    #[test]
    fn arbitrary_url_ssrf_policy_rejects_internal_targets() {
        use crate::utils::url_validator::{validate_outbound_url, OutboundUrlPolicy};
        let p = &OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS;
        assert!(validate_outbound_url("http://127.0.0.1/ggml-base.bin", p).is_err());
        assert!(validate_outbound_url("http://169.254.169.254/latest/meta-data", p).is_err());
        assert!(validate_outbound_url("http://10.0.0.5/x.bin", p).is_err());
        assert!(validate_outbound_url("https://huggingface.co/x/resolve/main/ggml-base.bin", p).is_ok());
    }

    #[test]
    fn redact_url_strips_credentials() {
        assert_eq!(
            redact_url("https://user:pass@example.com/ggml-base.bin"),
            "https://example.com/ggml-base.bin"
        );
        assert_eq!(
            redact_url("https://example.com/x.bin"),
            "https://example.com/x.bin"
        );
    }
}
