//! Server update checker — daily GitHub poll + in-memory cache.
//!
//! NOTIFICATION ONLY: never downloads or installs. Mirrors the reqwest/GitHub
//! polling pattern from `code_sandbox::version_manager` (same client setup,
//! headers, and a debug-only mirror seam for tests).

use std::sync::RwLock;
use std::time::Duration;

use once_cell::sync::Lazy;

use super::types::UpdateStatusResponse;

/// The GitHub repo whose releases we poll. (The repo URL is the one place
/// `ziee-chat` legitimately survives — see CLAUDE.md naming convention.)
const REPO: &str = "phibya/ziee-chat-new";

/// Process-lifetime cache, seeded with the running version.
static CACHE: Lazy<RwLock<UpdateStatusResponse>> = Lazy::new(|| {
    RwLock::new(UpdateStatusResponse {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: None,
        update_available: false,
        release_url: None,
        notes: None,
        checked_at: None,
        enabled: true,
    })
});

/// A snapshot of the cached status (cheap clone).
pub fn cached_status() -> UpdateStatusResponse {
    CACHE.read().expect("update cache poisoned").clone()
}

/// Record whether checks are enabled (set once at module init) so the UI can
/// show "checks disabled by operator config" without a separate endpoint.
pub fn set_enabled(enabled: bool) {
    CACHE.write().expect("update cache poisoned").enabled = enabled;
}

/// GitHub API base. Overridable in **debug** builds via
/// `SERVER_UPDATE_API_MIRROR` (the same testability seam as code_sandbox's
/// `CODE_SANDBOX_ROOTFS_MIRROR`); compiled out of release builds.
/// Returns `(base_url, is_mirror)`. `is_mirror` is true only when the debug-only
/// `SERVER_UPDATE_API_MIRROR` test seam is active — in release it is always
/// false (the seam is compiled out).
fn api_base() -> (String, bool) {
    #[cfg(debug_assertions)]
    {
        if let Ok(mirror) = std::env::var("SERVER_UPDATE_API_MIRROR")
            && !mirror.is_empty()
        {
            return (mirror.trim_end_matches('/').to_string(), true);
        }
    }
    ("https://api.github.com".to_string(), false)
}

/// Perform one check against GitHub's latest-release endpoint and update the
/// cache. Soft-fails on any network/parse error (logs `warn`, leaves the cache
/// intact) so a flaky network never breaks the server.
pub async fn check_once() {
    let (base, is_mirror) = api_base();
    let url = format!("{base}/repos/{REPO}/releases/latest");
    // https_only everywhere EXCEPT the loopback http test mirror.
    match fetch_latest(&url, !is_mirror).await {
        Ok(Some((tag, html_url, body))) => {
            let mut c = CACHE.write().expect("update cache poisoned");
            c.checked_at = Some(chrono::Utc::now().to_rfc3339());
            // Ignore non-semver tags so a garbage release name can't surface a
            // confusing "latest" in the UI or a false update banner.
            match semver_of(&tag) {
                Some(latest) => {
                    let available = is_newer(&latest, env!("CARGO_PKG_VERSION"));
                    c.latest_version = Some(latest);
                    c.update_available = available;
                    c.release_url = if html_url.is_empty() { None } else { Some(html_url) };
                    c.notes = body;
                    tracing::info!(update_available = available, "server update check complete");
                }
                None => {
                    c.latest_version = None;
                    c.update_available = false;
                    c.release_url = None;
                    c.notes = None;
                    tracing::warn!(%tag, "latest release tag is not semver; ignoring");
                }
            }
        }
        Ok(None) => tracing::debug!("server update check: no latest release"),
        Err(e) => tracing::warn!(error = %e, "server update check failed (soft)"),
    }
}

/// GET the `releases/latest` object and extract `(tag_name, html_url, body)`.
/// `https_only` is passed so the only-in-tests http loopback mirror can opt out.
async fn fetch_latest(
    url: &str,
    https_only: bool,
) -> Result<Option<(String, String, Option<String>)>, String> {
    let builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        // Refuse plaintext / http-redirect downgrades (matches version_manager.rs).
        // Only the SERVER_UPDATE_API_MIRROR loopback (tests) opts out.
        .https_only(https_only);
    let client = builder.build().map_err(|e| format!("client build: {e}"))?;
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "ziee/1.0")
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| format!("parse JSON: {e}"))?;
    Ok(extract_release(&v))
}

/// Pure extraction of `(tag_name, html_url, body?)` from a GitHub release JSON.
/// Returns `None` when there is no `tag_name` (e.g. a `{}` "no release" body).
/// An empty `body` normalizes to `None`. Pure → unit-testable without network.
fn extract_release(v: &serde_json::Value) -> Option<(String, String, Option<String>)> {
    let tag = v.get("tag_name").and_then(|t| t.as_str())?.to_string();
    let html_url = v
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    let body = v
        .get("body")
        .and_then(|b| b.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some((tag, html_url, body))
}

/// Normalize a release tag to a semver string, or `None` if it isn't semver.
/// Strips a leading `v` (`v1.2.3` → `1.2.3`); rejects e.g. `nightly-build`.
fn semver_of(tag: &str) -> Option<String> {
    let v = tag.trim_start_matches('v');
    semver::Version::parse(v).ok().map(|_| v.to_string())
}

/// `latest > current` per semver. Lenient: an unparseable version is never
/// treated as newer (so garbage from GitHub can't spam a false banner).
fn is_newer(latest: &str, current: &str) -> bool {
    match (semver::Version::parse(latest), semver::Version::parse(current)) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_older_equal() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("not-a-version", "0.1.0"));
    }

    #[test]
    fn prerelease_orders_below_release() {
        assert!(!is_newer("0.2.0-rc.1", "0.2.0"));
        assert!(is_newer("0.2.0", "0.2.0-rc.1"));
    }

    #[test]
    fn semver_of_accepts_versions_and_rejects_garbage() {
        assert_eq!(semver_of("v99.0.0").as_deref(), Some("99.0.0"));
        assert_eq!(semver_of("1.2.3").as_deref(), Some("1.2.3"));
        // Non-semver release names are rejected → check_once clears latest_version
        // (no false "update available" banner).
        assert!(semver_of("nightly-build").is_none());
        assert!(semver_of("latest").is_none());
        assert!(semver_of("").is_none());
    }

    #[test]
    fn set_enabled_reflects_in_cache() {
        // Drives the air-gapped path: when checks are disabled, the cached
        // status the endpoint serves reports `enabled: false`.
        set_enabled(false);
        assert!(!cached_status().enabled);
        set_enabled(true);
        assert!(cached_status().enabled);
    }

    #[test]
    fn config_default_enabled_true() {
        // An omitted `update_check` block deserializes to enabled = true.
        let c: crate::core::config::UpdateCheckConfig =
            serde_json::from_str("{}").expect("deserialize empty");
        assert!(c.enabled);
        assert!(crate::core::config::UpdateCheckConfig::default().enabled);
    }

    #[test]
    fn extract_release_full_shape() {
        let v = serde_json::json!({
            "tag_name": "v99.0.0",
            "html_url": "https://github.com/phibya/ziee-chat-new/releases/tag/v99.0.0",
            "body": "Release notes"
        });
        let (tag, url, body) = extract_release(&v).expect("has tag");
        assert_eq!(tag, "v99.0.0");
        assert_eq!(tag.trim_start_matches('v'), "99.0.0");
        assert!(url.ends_with("/v99.0.0"));
        assert_eq!(body.as_deref(), Some("Release notes"));
        // 99.0.0 stays newer than any plausible workspace version.
        assert!(is_newer("99.0.0", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn extract_release_handles_missing_and_empty() {
        // No tag_name → None (the GitHub "no release" / {} body).
        assert!(extract_release(&serde_json::json!({})).is_none());
        // Missing html_url → empty string; empty body → None.
        let (tag, url, body) = extract_release(&serde_json::json!({ "tag_name": "v1.0.0" }))
            .expect("has tag");
        assert_eq!(tag, "v1.0.0");
        assert_eq!(url, "");
        assert!(body.is_none());
        let (_, _, body2) =
            extract_release(&serde_json::json!({ "tag_name": "v1.0.0", "body": "" }))
                .expect("has tag");
        assert!(body2.is_none(), "empty body normalizes to None");
    }
}
