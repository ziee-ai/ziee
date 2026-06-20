//! Bundled CSL style registry.
//!
//! Journal `.csl` files dropped into `server/resources/csl/` (CC BY-SA 3.0; see
//! that dir's NOTICE) are baked into the binary via `include_dir!`. `Text`
//! formatting resolves a named style to a temp file pandoc can read; with no
//! bundled styles the formatter falls back to pandoc's built-in default
//! (Chicago author-date), so the feature works with zero bundled files.
//! pandoc 3.x ships CSL locales internally — no locale files are bundled.

use std::path::PathBuf;

use include_dir::{Dir, include_dir};

static CSL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/resources/csl");

/// Names of the bundled styles (filename without `.csl`), sorted.
pub fn list_styles() -> Vec<String> {
    let mut names: Vec<String> = CSL_DIR
        .files()
        .filter_map(|f| {
            let name = f.path().file_name()?.to_str()?;
            name.strip_suffix(".csl").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Resolve a style NAME to a real `.csl` path pandoc can read, extracting the
/// embedded bytes to a UNIQUE temp file per call. Returns `None` for an unknown
/// style (caller falls back to pandoc's default). The caller is responsible for
/// removing the returned file. A unique name per call avoids the TOCTOU where a
/// concurrent request reads a half-written shared file.
pub fn style_path(name: &str) -> Option<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let file = CSL_DIR.get_file(format!("{name}.csl"))?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::env::temp_dir().join(format!(
        "ziee-csl-{}-{n}-{name}.csl",
        std::process::id()
    ));
    std::fs::write(&out, file.contents()).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_style_returns_none() {
        assert!(style_path("definitely-not-a-real-style-xyz").is_none());
    }

    #[test]
    fn list_styles_does_not_panic() {
        // May be empty until journal .csl files are added — must not panic and
        // must never include the NOTICE file.
        let styles = list_styles();
        assert!(!styles.iter().any(|s| s == "NOTICE"));
    }
}
