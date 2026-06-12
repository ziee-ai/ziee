//! Shared model-file detection.
//!
//! This is the single source of truth for "given a flat list of files in a
//! repo (or a cloned working dir), which ones make up the model, and what
//! shape is it?". It is used by BOTH:
//!   - the `repository-files` listing endpoint (pre-download, over the
//!     Hugging Face / GitHub APIs), and
//!   - `handlers::uploads::determine_files_to_copy` (post-clone selection).
//!
//! The rules deliberately mirror how **mistral.rs** — the engine that
//! actually loads these models — auto-detects files, so what we download is
//! exactly what the engine expects to find on disk. Reference:
//!   - weight selection: `mistralrs-core/src/pipeline/paths.rs::get_model_paths`
//!     (prefer safetensors > pickle; take the WHOLE matching set; GGUF is an
//!     explicit per-file pick),
//!   - aux files: `mistralrs-core/src/pipeline/macros.rs::get_paths!`
//!     (`tokenizer.json`/`tekken.json`, `config.json`/`params.json`,
//!     `tokenizer_config.json`, `generation_config.json`,
//!     `preprocessor_config.json`, `processor_config.json`),
//!   - listing: `mistralrs-core/src/pipeline/hf.rs::list_repo_files`.
//!
//! Note we are intentionally *more inclusive* than mistral.rs on the
//! download side (we keep every `*.safetensors` shard, not just the
//! `model-*-of-*` naming): copying a superset is safe, and it closes the
//! "sharded safetensors without an index.json" gap where the old logic kept
//! only the single main file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The kind of weight container a repo holds. Priority gguf > safetensors >
/// pickle mirrors how the engine decides what to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelShape {
    Gguf,
    Safetensors,
    Pickle,
    Unknown,
}

/// Coarse role of a single file, surfaced to the UI for grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    Weight,
    Index,
    Config,
    Tokenizer,
    Vocab,
    Other,
}

/// Result of detecting the model file set within a flat listing.
#[derive(Debug, Clone)]
pub struct DetectedFiles {
    pub shape: ModelShape,
    /// Weight files for the detected shape (full paths, as given).
    pub weights: Vec<String>,
    /// A reasonable default "main" file name for the download request.
    pub suggested_main: Option<String>,
}

// ---------------------------------------------------------------------------
// Low-level predicates (operate on the basename, case-insensitively)
// ---------------------------------------------------------------------------

/// Last path segment.
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn lc(path: &str) -> String {
    basename(path).to_lowercase()
}

pub fn is_gguf(path: &str) -> bool {
    lc(path).ends_with(".gguf")
}

/// Any safetensors weight file. Inclusive on purpose (see module docs).
pub fn is_safetensors_weight(path: &str) -> bool {
    lc(path).ends_with(".safetensors")
}

/// pickle / pytorch weight file (`.bin` / `.pt` / `.pth`).
pub fn is_pickle_weight(path: &str) -> bool {
    let b = lc(path);
    b.ends_with(".bin") || b.ends_with(".pt") || b.ends_with(".pth")
}

/// Non-weight pickle artifacts HF training repos ship (`training_args.bin`,
/// `optimizer.pt`, `rng_state*.pth`, `scheduler.pt`, …). They match
/// [`is_pickle_weight`] by extension but must NOT be downloaded as weights.
pub fn is_pickle_noise(path: &str) -> bool {
    let b = lc(path);
    matches!(b.as_str(), "training_args.bin")
        || b.starts_with("optimizer")
        || b.starts_with("scheduler")
        || b.starts_with("rng_state")
}

pub fn is_index_file(path: &str) -> bool {
    lc(path).ends_with(".index.json")
}

/// Config / tokenizer / index / vocab files the engine loads alongside the
/// weights. Superset of mistral.rs's `get_paths!` set plus the
/// sentencepiece/BPE vocab files our older logic already kept.
pub fn is_aux_file(path: &str) -> bool {
    // A weight is never aux, regardless of how its basename happens to read
    // (e.g. a hypothetical `config_model.safetensors`). Mirrors classify()'s
    // weight-first ordering. Index files are NOT weights, so the
    // is_index_file check below still applies.
    if is_gguf(path) || is_safetensors_weight(path) || is_pickle_weight(path) {
        return false;
    }
    let b = lc(path);
    if is_index_file(path) {
        return true;
    }
    matches!(
        b.as_str(),
        "config.json"
            | "params.json"
            | "generation_config.json"
            | "tokenizer.json"
            | "tekken.json"
            | "tokenizer.model"
            | "tokenizer_config.json"
            | "special_tokens_map.json"
            | "preprocessor_config.json"
            | "processor_config.json"
            | "vocab.json"
            | "vocab.txt"
            | "merges.txt"
            | "spiece.model"
    ) || b.ends_with("config.json")   // adapter_config.json, quantize_config.json, ...
        || b.ends_with("tokenizer.json")
        || b.starts_with("tokenizer_")
        || b.starts_with("config_")
        || b.starts_with("chat_template")
}

/// File extension → our `FileFormat` string (matches `models::FileFormat::as_str`).
pub fn file_format_for(path: &str) -> Option<&'static str> {
    let b = lc(path);
    if b.ends_with(".gguf") {
        Some("gguf")
    } else if b.ends_with(".safetensors") {
        Some("safetensors")
    } else if b.ends_with(".bin") || b.ends_with(".pt") || b.ends_with(".pth") {
        Some("pytorch")
    } else {
        None
    }
}

/// Coarse role for UI grouping.
pub fn classify(path: &str) -> FileRole {
    if is_index_file(path) {
        return FileRole::Index;
    }
    if is_gguf(path) || is_safetensors_weight(path) || is_pickle_weight(path) {
        return FileRole::Weight;
    }
    let b = lc(path);
    // Keep the Config/Tokenizer predicates in lockstep with `is_aux_file`
    // so a file the downloader fetches as aux is never labelled "Other".
    if b == "params.json"
        || b == "generation_config.json"
        || b == "preprocessor_config.json"
        || b == "processor_config.json"
        || b.ends_with("config.json")   // config.json, adapter_config.json, quantize_config.json
        || b.starts_with("config_")
        || b.starts_with("chat_template")
    {
        return FileRole::Config;
    }
    if b == "tekken.json"
        || b == "tokenizer.model"
        || b == "special_tokens_map.json"
        || b == "spiece.model"
        || b.ends_with("tokenizer.json")   // tokenizer.json, slow_tokenizer.json
        || b.starts_with("tokenizer_")
    {
        return FileRole::Tokenizer;
    }
    if b == "vocab.json" || b == "vocab.txt" || b == "merges.txt" {
        return FileRole::Vocab;
    }
    FileRole::Other
}

// ---------------------------------------------------------------------------
// Sharding helpers
// ---------------------------------------------------------------------------

/// For a sharded weight name like `model-00001-of-00003.safetensors` (or the
/// `_00001_of_00003_` variant), return `(prefix, total)` — e.g.
/// `("model", "00003")`. `None` when the name isn't a shard. The total is
/// part of the key so two distinct shard sets that share a prefix but differ
/// in shard count don't get merged.
pub fn shard_key(path: &str) -> Option<(String, String)> {
    let b = basename(path);
    // Match the `-of-` / `_of_` separator case-insensitively (filenames are
    // ASCII, so lowercased byte offsets line up with the original).
    let bl = b.to_ascii_lowercase();
    for sep in ["-of-", "_of_"] {
        let dash = if sep == "-of-" { '-' } else { '_' };
        if let Some(of_pos) = bl.find(sep) {
            let before = &b[..of_pos];
            // before must end with `<dash><digits>` and there must be a
            // `<digits>` immediately after the separator.
            if let Some(dpos) = before.rfind(dash) {
                let shard_no = &before[dpos + 1..];
                let after = &b[of_pos + sep.len()..];
                let total: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !shard_no.is_empty()
                    && shard_no.chars().all(|c| c.is_ascii_digit())
                    && !total.is_empty()
                {
                    return Some((before[..dpos].to_string(), total));
                }
            }
        }
    }
    None
}

/// The shard prefix only (`model` from `model-00001-of-00003.safetensors`).
pub fn shard_prefix(path: &str) -> Option<String> {
    shard_key(path).map(|(p, _)| p)
}

fn same_shard_group(candidate: &str, key: &(String, String), ext: &str) -> bool {
    if !lc(candidate).ends_with(ext) {
        return false;
    }
    match shard_key(candidate) {
        Some((p, t)) => p.eq_ignore_ascii_case(&key.0) && t == key.1,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Detect the model file set from a flat listing of repo file paths.
pub fn detect_weight_set(files: &[String]) -> DetectedFiles {
    let ggufs: Vec<String> = files.iter().filter(|f| is_gguf(f)).cloned().collect();
    let safes: Vec<String> = files
        .iter()
        .filter(|f| is_safetensors_weight(f))
        .cloned()
        .collect();
    let pickles: Vec<String> = files
        .iter()
        .filter(|f| is_pickle_weight(f) && !is_pickle_noise(f))
        .cloned()
        .collect();
    let aux: Vec<String> = files.iter().filter(|f| is_aux_file(f)).cloned().collect();

    let (shape, mut weights) = if !ggufs.is_empty() {
        (ModelShape::Gguf, ggufs)
    } else if !safes.is_empty() {
        (ModelShape::Safetensors, safes)
    } else if !pickles.is_empty() {
        (ModelShape::Pickle, pickles)
    } else {
        (ModelShape::Unknown, Vec::new())
    };
    weights.sort();

    let suggested_main = suggest_main(shape, &weights, &aux);

    DetectedFiles {
        shape,
        weights,
        suggested_main,
    }
}

fn suggest_main(shape: ModelShape, weights: &[String], aux: &[String]) -> Option<String> {
    match shape {
        ModelShape::Safetensors | ModelShape::Pickle => {
            // Prefer the index (loaders use it to map shards), then a
            // canonical single file, then the first shard.
            if let Some(idx) = aux.iter().find(|f| is_index_file(f)) {
                return Some(basename(idx).to_string());
            }
            let canonical = if shape == ModelShape::Safetensors {
                "model.safetensors"
            } else {
                "pytorch_model.bin"
            };
            if let Some(c) = weights.iter().find(|f| lc(f) == canonical) {
                return Some(basename(c).to_string());
            }
            weights.first().map(|f| basename(f).to_string())
        }
        ModelShape::Gguf => {
            // Default to a widely-used balanced quant if present, else the
            // first gguf. (Sizes aren't known here; the listing endpoint may
            // refine this with a smallest-file heuristic.)
            if let Some(q) = weights
                .iter()
                .find(|f| lc(f).contains("q4_k_m"))
                .or_else(|| weights.iter().find(|f| lc(f).contains("q4_0")))
            {
                return Some(basename(q).to_string());
            }
            weights.first().map(|f| basename(f).to_string())
        }
        ModelShape::Unknown => None,
    }
}

/// Select which files to actually download/copy, given the full listing and
/// the requested `main_filename`. Mirrors mistral.rs: take the whole weight
/// set for the chosen shape (so sharded safetensors without an index.json
/// still get every shard) plus all aux files.
///
/// Returns an error string when no weight file can be found.
pub fn select_download_files(source_files: &[String], main_filename: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let main = main_filename.trim();

    // GGUF: a single quant pick. Keep that file plus its shard siblings only
    // (NOT other quants).
    if main.to_lowercase().ends_with(".gguf") {
        let main_present = source_files.iter().any(|f| basename(f) == basename(main));
        if !main_present {
            return Err(format!("'{main}' not found in source directory"));
        }
        if let Some(key) = shard_key(main) {
            for f in source_files {
                if same_shard_group(f, &key, ".gguf") {
                    out.push(f.clone());
                }
            }
        } else {
            // copy the exact main entry (preserve its path)
            if let Some(f) = source_files.iter().find(|f| basename(f) == basename(main)) {
                out.push(f.clone());
            }
        }
    } else if is_safetensors_weight(main) {
        // The named main must exist (a typo on the upload-commit path must
        // error, not silently commit some other weight).
        if !source_files.iter().any(|f| basename(f) == basename(main)) {
            return Err(format!("'{main}' not found in source directory"));
        }
        // Honor the chosen shape: take the WHOLE safetensors set (so a
        // sharded repo without an index.json still gets every shard).
        out.extend(source_files.iter().filter(|f| is_safetensors_weight(f)).cloned());
    } else if is_pickle_weight(main) {
        // A training artifact (optimizer/scheduler/rng/training_args) is never
        // a model weight — reject it with a clear message.
        if is_pickle_noise(main) {
            return Err(format!("'{main}' is a training artifact, not a model weight"));
        }
        if !source_files.iter().any(|f| basename(f) == basename(main)) {
            return Err(format!("'{main}' not found in source directory"));
        }
        // A .bin/.pt/.pth pick stays pickle even if safetensors are also
        // present (don't silently override the user's explicit choice).
        // Exclude training artifacts (optimizer/scheduler/rng/training_args).
        out.extend(
            source_files
                .iter()
                .filter(|f| is_pickle_weight(f) && !is_pickle_noise(f))
                .cloned(),
        );
    } else {
        // main isn't a recognizable weight (e.g. an index.json, or empty) —
        // fall back to the detected shape's whole set.
        let det = detect_weight_set(source_files);
        match det.shape {
            ModelShape::Safetensors | ModelShape::Pickle => {
                out.extend(det.weights.iter().cloned());
            }
            ModelShape::Gguf | ModelShape::Unknown => {
                // Listing is gguf/unknown but the request wasn't a .gguf main
                // — fall back to the named main file if present.
                if !main.is_empty()
                    && let Some(f) = source_files.iter().find(|f| basename(f) == basename(main))
                {
                    out.push(f.clone());
                }
            }
        }
    }

    // Always include config/tokenizer/index files.
    for f in source_files {
        if is_aux_file(f) && !out.iter().any(|x| x == f) {
            out.push(f.clone());
        }
    }

    out.sort();
    out.dedup();

    if !out.iter().any(|f| is_gguf(f) || is_safetensors_weight(f) || is_pickle_weight(f)) {
        let repo_has_weights = source_files
            .iter()
            .any(|f| is_gguf(f) || is_safetensors_weight(f) || is_pickle_weight(f));
        return Err(if repo_has_weights {
            format!("'{main}' does not match any model weight file in the repository")
        } else {
            "No model weight files found in source directory".to_string()
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_sharded_safetensors_with_index() {
        let files = v(&[
            "model.safetensors.index.json",
            "model-00001-of-00003.safetensors",
            "model-00002-of-00003.safetensors",
            "model-00003-of-00003.safetensors",
            "config.json",
            "tokenizer.json",
        ]);
        let d = detect_weight_set(&files);
        assert_eq!(d.shape, ModelShape::Safetensors);
        assert_eq!(d.weights.len(), 3);
        assert_eq!(d.suggested_main.as_deref(), Some("model.safetensors.index.json"));
        // config/tokenizer travel with the weights on copy.
        let copy = select_download_files(&files, "model.safetensors.index.json").unwrap();
        assert!(copy.contains(&"config.json".to_string()));
        assert!(copy.contains(&"tokenizer.json".to_string()));
        assert_eq!(copy.iter().filter(|f| f.ends_with(".safetensors")).count(), 3);
    }

    #[test]
    fn sharded_safetensors_without_index_still_grabs_all_shards() {
        // The gap-A case: no index.json present.
        let files = v(&[
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
            "config.json",
            "tokenizer.json",
        ]);
        let copy = select_download_files(&files, "model-00001-of-00002.safetensors").unwrap();
        assert!(copy.contains(&"model-00001-of-00002.safetensors".to_string()));
        assert!(copy.contains(&"model-00002-of-00002.safetensors".to_string()));
        assert!(copy.contains(&"config.json".to_string()));
        assert!(copy.contains(&"tokenizer.json".to_string()));
    }

    #[test]
    fn single_safetensors() {
        let files = v(&["model.safetensors", "config.json", "tokenizer.json"]);
        let d = detect_weight_set(&files);
        assert_eq!(d.shape, ModelShape::Safetensors);
        assert_eq!(d.suggested_main.as_deref(), Some("model.safetensors"));
        let copy = select_download_files(&files, "model.safetensors").unwrap();
        assert_eq!(copy.len(), 3);
    }

    #[test]
    fn consolidated_safetensors_is_a_weight() {
        let files = v(&["consolidated.safetensors", "params.json", "tekken.json"]);
        let d = detect_weight_set(&files);
        assert_eq!(d.shape, ModelShape::Safetensors);
        // ...and the whole set (weight + Mistral-native aux) is selected.
        let copy = select_download_files(&files, "consolidated.safetensors").unwrap();
        assert!(copy.contains(&"consolidated.safetensors".to_string()));
        assert!(copy.contains(&"params.json".to_string()));
        assert!(copy.contains(&"tekken.json".to_string()));
    }

    #[test]
    fn missing_main_filename_errors() {
        // A typo'd main on the upload-commit path must error, not silently
        // commit a different weight.
        let files = v(&["model.safetensors", "config.json"]);
        assert!(select_download_files(&files, "nope.safetensors").is_err());
        assert!(select_download_files(&files, "nope.bin").is_err());
    }

    #[test]
    fn pickle_noise_is_excluded() {
        // A training repo: real weight + optimizer/scheduler/training noise.
        let files = v(&[
            "pytorch_model.bin",
            "optimizer.pt",
            "scheduler.pt",
            "rng_state.pth",
            "training_args.bin",
            "config.json",
        ]);
        let copy = select_download_files(&files, "pytorch_model.bin").unwrap();
        assert!(copy.contains(&"pytorch_model.bin".to_string()));
        assert!(!copy.iter().any(|f| f == "optimizer.pt" || f == "scheduler.pt"));
        assert!(!copy.iter().any(|f| f == "rng_state.pth" || f == "training_args.bin"));
    }

    #[test]
    fn gguf_multi_quant_picks_only_chosen() {
        let files = v(&[
            "tinyllama-Q4_K_M.gguf",
            "tinyllama-Q5_K_M.gguf",
            "tinyllama-Q8_0.gguf",
            "config.json",
        ]);
        let d = detect_weight_set(&files);
        assert_eq!(d.shape, ModelShape::Gguf);
        assert_eq!(d.suggested_main.as_deref(), Some("tinyllama-Q4_K_M.gguf"));
        let copy = select_download_files(&files, "tinyllama-Q5_K_M.gguf").unwrap();
        // only the chosen quant (no shards) + aux
        assert!(copy.contains(&"tinyllama-Q5_K_M.gguf".to_string()));
        assert!(!copy.contains(&"tinyllama-Q4_K_M.gguf".to_string()));
        assert!(!copy.contains(&"tinyllama-Q8_0.gguf".to_string()));
    }

    #[test]
    fn gguf_sharded_keeps_siblings() {
        let files = v(&[
            "big-Q4_K_M-00001-of-00002.gguf",
            "big-Q4_K_M-00002-of-00002.gguf",
            "other-Q8_0.gguf",
        ]);
        let copy =
            select_download_files(&files, "big-Q4_K_M-00001-of-00002.gguf").unwrap();
        assert!(copy.contains(&"big-Q4_K_M-00001-of-00002.gguf".to_string()));
        assert!(copy.contains(&"big-Q4_K_M-00002-of-00002.gguf".to_string()));
        assert!(!copy.contains(&"other-Q8_0.gguf".to_string()));
    }

    #[test]
    fn pickle_fallback() {
        let files = v(&["pytorch_model.bin", "config.json", "vocab.json", "merges.txt"]);
        let d = detect_weight_set(&files);
        assert_eq!(d.shape, ModelShape::Pickle);
        let copy = select_download_files(&files, "pytorch_model.bin").unwrap();
        assert!(copy.contains(&"pytorch_model.bin".to_string()));
        assert!(copy.contains(&"merges.txt".to_string()));
    }

    #[test]
    fn gguf_preferred_over_safetensors_for_shape() {
        let files = v(&["model.safetensors", "model-Q4_K_M.gguf", "config.json"]);
        assert_eq!(detect_weight_set(&files).shape, ModelShape::Gguf);
    }

    #[test]
    fn safetensors_preferred_over_pickle_for_shape() {
        // Offline pin for the priority the live tiny-random-gpt2 test relies on.
        let files = v(&["model.safetensors", "pytorch_model.bin", "config.json"]);
        assert_eq!(detect_weight_set(&files).shape, ModelShape::Safetensors);
    }

    #[test]
    fn shard_prefix_is_case_insensitive() {
        assert_eq!(
            shard_prefix("model-00001-OF-00003.safetensors").as_deref(),
            Some("model")
        );
    }

    #[test]
    fn shard_sets_with_same_prefix_different_total_dont_merge() {
        let files = v(&[
            "m-00001-of-00002.gguf",
            "m-00002-of-00002.gguf",
            "m-00001-of-00003.gguf", // a different set (total 3), same prefix
        ]);
        let copy = select_download_files(&files, "m-00001-of-00002.gguf").unwrap();
        assert!(copy.contains(&"m-00001-of-00002.gguf".to_string()));
        assert!(copy.contains(&"m-00002-of-00002.gguf".to_string()));
        assert!(!copy.contains(&"m-00001-of-00003.gguf".to_string()));
    }

    #[test]
    fn pickle_noise_as_main_errors() {
        let files = v(&["pytorch_model.bin", "optimizer.pt", "config.json"]);
        assert!(select_download_files(&files, "optimizer.pt").is_err());
    }

    #[test]
    fn classify_and_format() {
        assert_eq!(classify("model-00001-of-00003.safetensors"), FileRole::Weight);
        assert_eq!(classify("model.safetensors.index.json"), FileRole::Index);
        assert_eq!(classify("config.json"), FileRole::Config);
        assert_eq!(classify("tokenizer.json"), FileRole::Tokenizer);
        assert_eq!(classify("merges.txt"), FileRole::Vocab);
        assert_eq!(classify("README.md"), FileRole::Other);
        assert_eq!(file_format_for("x.gguf"), Some("gguf"));
        assert_eq!(file_format_for("x.safetensors"), Some("safetensors"));
        assert_eq!(file_format_for("x.bin"), Some("pytorch"));
        assert_eq!(file_format_for("x.txt"), None);
    }

    #[test]
    fn shard_prefix_parsing() {
        assert_eq!(
            shard_prefix("model-00001-of-00003.safetensors").as_deref(),
            Some("model")
        );
        assert_eq!(
            shard_prefix("foo_00001_of_00010_.bin").as_deref(),
            Some("foo")
        );
        assert_eq!(shard_prefix("model.safetensors"), None);
    }

    #[test]
    fn unknown_when_no_weights() {
        let files = v(&["README.md", "config.json"]);
        assert_eq!(detect_weight_set(&files).shape, ModelShape::Unknown);
        assert!(select_download_files(&files, "README.md").is_err());
    }

    // Pure-function path tolerance ONLY. Neither production caller currently
    // produces a path containing '/' (the listing endpoint filters to
    // top-level files, and the clone/upload copy reads a flat dir), so this
    // nested-input shape is not reachable end-to-end today — it just pins
    // that the selector itself is basename-correct if that ever changes.
    #[test]
    fn nested_paths_use_basename() {
        let files = v(&["sub/model.safetensors", "sub/config.json"]);
        let copy = select_download_files(&files, "model.safetensors").unwrap();
        assert!(copy.contains(&"sub/model.safetensors".to_string()));
        assert!(copy.contains(&"sub/config.json".to_string()));
    }

    #[test]
    fn gguf_main_not_present_errors() {
        // Listing IS gguf, but the requested quant isn't in it.
        let files = v(&["a-Q4_K_M.gguf", "config.json"]);
        assert!(select_download_files(&files, "missing-Q8_0.gguf").is_err());
    }

    #[test]
    fn explicit_pickle_main_is_honored_over_safetensors() {
        // Both shapes present; the user explicitly picked the .bin — it must
        // win (not be silently overridden by the safetensors set).
        let files = v(&["model.safetensors", "pytorch_model.bin", "config.json"]);
        let copy = select_download_files(&files, "pytorch_model.bin").unwrap();
        assert!(copy.contains(&"pytorch_model.bin".to_string()));
        assert!(!copy.contains(&"model.safetensors".to_string()));
    }

    #[test]
    fn multi_independent_safetensors_grabs_whole_set() {
        // By design (mistral.rs-parity sharded-without-index fix) a
        // safetensors pick pulls the whole safetensors set, even non-shard
        // independent files. Pinned so the divergence is intentional.
        let files = v(&["model.safetensors", "model.fp16.safetensors", "config.json"]);
        let copy = select_download_files(&files, "model.safetensors").unwrap();
        assert!(copy.contains(&"model.safetensors".to_string()));
        assert!(copy.contains(&"model.fp16.safetensors".to_string()));
    }

    #[test]
    fn consolidated_safetensors_suggested_main() {
        let files = v(&["consolidated.safetensors", "params.json"]);
        assert_eq!(
            detect_weight_set(&files).suggested_main.as_deref(),
            Some("consolidated.safetensors"),
        );
    }

    #[test]
    fn adapter_and_quantize_config_are_aux() {
        assert!(is_aux_file("adapter_config.json"));
        assert!(is_aux_file("quantize_config.json"));
        assert!(is_aux_file("quantization_config.json"));
    }
}
