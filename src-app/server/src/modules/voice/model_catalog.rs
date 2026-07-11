//! Runtime whisper-model catalog: list the downloadable `ggml-*.bin` models from
//! the admin-configured source repo (default `ggerganov/whisper.cpp`) via the
//! HuggingFace tree API, reading each file's git-LFS `oid` (= sha256) so a
//! download can be verified against a source-of-truth digest.
//!
//! This mirrors the engine-version "available list" pattern (upstream fetch at
//! runtime), NOT a hardcoded constant — so new upstream models appear
//! automatically. **Graceful degradation:** a fetch failure yields an empty list
//! + `source_reachable=false`; upload + arbitrary-URL + installed models are
//! unaffected.
//!
//! Trust boundary: the source repo is ADMIN-configured (may be an internal
//! mirror), so the list fetch uses a trusted client (no SSRF restriction) —
//! distinct from the user-supplied arbitrary-URL download in `model.rs`, which is
//! SSRF-validated. (Mirrors web_search's trusted-SearXNG vs strict-page-fetch.)

use std::time::Duration;

use serde::Deserialize;

use crate::common::AppError;

/// The HF tree-API base. Overridable in **debug builds only** via
/// `WHISPER_CATALOG_MIRROR` so a test can serve a fixture tree from loopback
/// (mirrors `WHISPER_MODEL_MIRROR`).
fn hf_api_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("WHISPER_CATALOG_MIRROR") {
        if !base.is_empty() {
            return base.trim_end_matches('/').to_string();
        }
    }
    "https://huggingface.co/api/models".to_string()
}

/// Build the tree-listing URL for a source repo. An `owner/repo` slug resolves to
/// the HF tree API; a full `https://` base is used as-is (best-effort for a
/// non-HF mirror that mimics the tree schema).
fn tree_url(source_repo: &str) -> String {
    let r = source_repo.trim();
    if r.starts_with("https://") {
        format!("{}/tree/main", r.trim_end_matches('/'))
    } else {
        format!("{}/{}/tree/main", hf_api_base(), r)
    }
}

/// The direct file-download base for a source repo. Overridable in **debug
/// builds** via `WHISPER_MODEL_MIRROR` so a test serves fixtures from loopback.
fn download_base(source_repo: &str) -> String {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("WHISPER_MODEL_MIRROR") {
        if !base.is_empty() {
            return base.trim_end_matches('/').to_string();
        }
    }
    let r = source_repo.trim();
    if r.starts_with("https://") {
        r.trim_end_matches('/').to_string()
    } else {
        format!("https://huggingface.co/{r}/resolve/main")
    }
}

/// The direct download URL for a model file from the configured source.
pub fn download_url(source_repo: &str, filename: &str) -> String {
    format!("{}/{}", download_base(source_repo), filename)
}

/// The direct download URL for an arbitrary HF `owner/repo` + file (user-supplied).
pub fn hf_repo_url(repository: &str, filename: &str) -> String {
    format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repository.trim_matches('/'),
        filename
    )
}

/// One entry from the HF tree API. LFS blobs carry `lfs.oid` (the sha256).
#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    kind: Option<String>,
    path: String,
    #[serde(default)]
    size: Option<i64>,
    #[serde(default)]
    lfs: Option<HfLfs>,
}

#[derive(Debug, Deserialize)]
struct HfLfs {
    oid: Option<String>,
    size: Option<i64>,
}

/// A resolved catalog entry (source-of-truth digest + metadata).
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub name: String,
    pub filename: String,
    pub size_bytes: Option<i64>,
    pub sha256: Option<String>,
    pub english_only: bool,
    pub quantization: Option<String>,
}

/// Derive the short model name from a `ggml-<name>.bin` filename.
pub fn name_from_filename(filename: &str) -> Option<String> {
    let stem = filename
        .strip_prefix("ggml-")?
        .strip_suffix(".bin")
        .or_else(|| filename.strip_prefix("ggml-")?.strip_suffix(".gguf"))?;
    if stem.is_empty() { None } else { Some(stem.to_string()) }
}

fn detect_quantization(name: &str) -> Option<String> {
    // whisper.cpp quantized files carry a `-q5_1` / `-q8_0` / `-q5_0` segment.
    name.split('-')
        .find(|seg| {
            let s = seg.to_ascii_lowercase();
            s.starts_with('q') && s.len() >= 3 && s[1..].chars().all(|c| c.is_ascii_alphanumeric())
        })
        .map(|s| s.to_string())
}

/// Parse an HF tree-API JSON body into catalog entries (filter `ggml-*.bin`).
/// Pure — unit-tested against a fixture body.
pub fn parse_tree(body: &str) -> Vec<CatalogEntry> {
    let entries: Vec<HfTreeEntry> = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    entries
        .into_iter()
        .filter(|e| e.kind.as_deref() != Some("directory"))
        .filter(|e| e.path.starts_with("ggml-") && e.path.ends_with(".bin"))
        .filter_map(|e| {
            let name = name_from_filename(&e.path)?;
            let sha256 = e.lfs.as_ref().and_then(|l| l.oid.clone());
            let size_bytes = e.lfs.as_ref().and_then(|l| l.size).or(e.size);
            let english_only = name.ends_with(".en");
            let quantization = detect_quantization(&name);
            Some(CatalogEntry {
                name,
                filename: e.path,
                size_bytes,
                sha256,
                english_only,
                quantization,
            })
        })
        .collect()
}

/// Fetch + parse the catalog from the configured source. Returns
/// `(entries, reachable)`; a network/HTTP failure yields `(vec![], false)`
/// (graceful degradation), never an error to the caller.
pub async fn fetch_catalog(source_repo: &str) -> (Vec<CatalogEntry>, bool) {
    match fetch_catalog_inner(source_repo).await {
        Ok(entries) => (entries, true),
        Err(e) => {
            tracing::warn!("voice: model catalog fetch from {source_repo:?} failed: {e}");
            (Vec::new(), false)
        }
    }
}

async fn fetch_catalog_inner(source_repo: &str) -> Result<Vec<CatalogEntry>, AppError> {
    let url = tree_url(source_repo);
    // Trusted client (admin-configured source may be internal); no proxy so a
    // loopback mirror/test fixture is reachable regardless of env proxy vars.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()
        .map_err(AppError::internal_with_id)?;
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("catalog request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "catalog fetch returned HTTP {} for {url}",
            resp.status()
        )));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::internal_error(format!("catalog read failed: {e}")))?;
    Ok(parse_tree(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"type":"file","path":"README.md","size":1234},
        {"type":"file","path":"ggml-base.bin","size":147951465,
         "lfs":{"oid":"60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe","size":147951465}},
        {"type":"file","path":"ggml-base.en.bin",
         "lfs":{"oid":"a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002","size":147964211}},
        {"type":"file","path":"ggml-large-v3-turbo-q5_0.bin",
         "lfs":{"oid":"deadbeef","size":574041195}},
        {"type":"directory","path":"examples"}
    ]"#;

    #[test]
    fn parses_and_filters_ggml_files_with_oid() {
        let entries = parse_tree(SAMPLE);
        assert_eq!(entries.len(), 3, "only the three ggml-*.bin files");
        let base = entries.iter().find(|e| e.name == "base").unwrap();
        assert_eq!(base.sha256.as_deref(), Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"));
        assert!(!base.english_only);
        assert!(base.quantization.is_none());
        let en = entries.iter().find(|e| e.name == "base.en").unwrap();
        assert!(en.english_only);
        let q = entries.iter().find(|e| e.name == "large-v3-turbo-q5_0").unwrap();
        assert_eq!(q.quantization.as_deref(), Some("q5_0"));
    }

    #[test]
    fn malformed_body_degrades_to_empty() {
        assert!(parse_tree("not json").is_empty());
        assert!(parse_tree("{}").is_empty());
    }

    #[test]
    fn name_from_filename_strips_affixes() {
        assert_eq!(name_from_filename("ggml-small.en.bin").as_deref(), Some("small.en"));
        assert_eq!(name_from_filename("other.bin"), None);
    }

    #[test]
    fn tree_url_handles_slug_and_url() {
        assert!(tree_url("ggerganov/whisper.cpp").ends_with("/ggerganov/whisper.cpp/tree/main"));
        assert_eq!(
            tree_url("https://hf.internal/repo/"),
            "https://hf.internal/repo/tree/main"
        );
    }
}
