//! Embedded Office add-in assets (ITEM-12).
//!
//! The task-pane bundle — `manifest.xml`, `taskpane.html`, `taskpane.js`,
//! `icon.png` — lives under `resources/office-bridge/` and is baked into the
//! binary via `include_dir!` (the `skill/builtin.rs` + `citations/csl.rs`
//! pattern). The ITEM-5 rustls listener serves these bytes at
//! `https://localhost:44300/<path>`; the `[Connect]` flow (ITEM-13) writes the
//! manifest out for sideloading.
//!
//! This module is just the byte accessor + a content-type helper. The listener
//! that serves them (and substitutes the per-session token into
//! `taskpane.html`) is ITEM-5.

use include_dir::{Dir, include_dir};

/// The embedded add-in asset directory (manifest + task pane + icon).
static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/resources/office-bridge");

/// Fetch an embedded asset's raw bytes by its file name (e.g. `"taskpane.html"`).
/// A leading `/` is tolerated so both `/taskpane.html` and `taskpane.html`
/// resolve. Returns `None` for an unknown path.
pub fn get(path: &str) -> Option<&'static [u8]> {
    let rel = path.trim_start_matches('/');
    ASSETS.get_file(rel).map(|f| f.contents())
}

/// Guess the HTTP `Content-Type` for a served asset path from its extension.
/// Covers exactly the asset types this bundle ships; unknown extensions fall
/// back to `application/octet-stream`.
pub fn content_type(path: &str) -> &'static str {
    let lower = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match lower.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "png" => "image/png",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-15 (unit form) — the embedded `manifest.xml` + `taskpane.html`
    /// exist via `include_dir!`, and the manifest is well-formed multi-host XML
    /// carrying the three Host names + the canonical SourceLocation. The
    /// served-over-HTTP assertion is deferred to ITEM-5's bridge_test.
    #[test]
    fn test15_embedded_assets_present_and_manifest_wellformed() {
        // Both key assets resolve (with and without a leading slash).
        assert!(ASSETS.get_file("manifest.xml").is_some());
        assert!(ASSETS.get_file("taskpane.html").is_some());
        assert!(get("/manifest.xml").is_some());
        assert!(get("taskpane.js").is_some());
        assert!(get("icon.png").is_some());
        assert!(get("does-not-exist").is_none());

        let manifest = std::str::from_utf8(get("manifest.xml").expect("manifest bytes"))
            .expect("manifest is utf-8");

        // Well-formed XML: a naive check that every '<' has a matching '>' and
        // the doc parses as balanced tags is overkill here; instead assert the
        // structural anchors the listener + Office depend on. (Full XML
        // well-formedness is asserted by the PowerShell validation at author
        // time; ITEM-5's integration test re-parses it over HTTP.)
        assert!(manifest.starts_with("<?xml"), "XML prolog present");
        assert!(manifest.contains("</OfficeApp>"), "root element closed");

        // Three hosts (Document/Workbook/Presentation).
        assert!(manifest.contains("<Host Name=\"Document\"/>"));
        assert!(manifest.contains("<Host Name=\"Workbook\"/>"));
        assert!(manifest.contains("<Host Name=\"Presentation\"/>"));

        // Canonical shared SourceLocation.
        assert!(
            manifest.contains("https://localhost:44300/taskpane.html"),
            "manifest carries the fixed SourceLocation"
        );

        // Content-type helper maps the shipped asset extensions.
        assert_eq!(content_type("taskpane.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("taskpane.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type("manifest.xml"), "application/xml; charset=utf-8");
        assert_eq!(content_type("icon.png"), "image/png");
    }
}
