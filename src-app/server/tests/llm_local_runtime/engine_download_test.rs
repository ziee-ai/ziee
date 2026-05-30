//! Tier 2 — engine-binary download from the mock release repo.
//!
//! Exercises the FULL download pipeline (resolve → fetch → extract →
//! cache → register) against the loopback `MockReleaseServer`, plus
//! version CRUD + permissions.

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::mock_release;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;

/// The engine downloads from the mock, registers a version row, and
/// shows up in the (engine-filtered) version list. The previous
/// `allow_unsigned_downloads` supply-chain gate has been removed —
/// downloads now proceed unconditionally (cosign verify in the runtime
/// crate logs a warning when the sibling `.sig` is missing but no
/// longer blocks).
#[tokio::test]
async fn download_engine_from_mock_succeeds() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let versions = body["versions"].as_array().expect("versions array");
    assert!(
        versions
            .iter()
            .any(|v| v["id"].as_str() == Some(version_id.to_string().as_str())),
        "downloaded version should appear in the list: {body}"
    );
}

/// A re-download is idempotent (cache hit) and still returns 200.
#[tokio::test]
async fn download_idempotent_on_second_call() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let v1 = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    // Second explicit download of the same coordinates.
    let payload = json!({
        "engine": "llamacpp",
        "version": mock.version,
        "platform": mock.platform,
        "arch": mock.arch,
        "backend": "cpu",
    });
    let resp = reqwest::Client::new()
        .post(mock.server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "re-download should be a cache hit");
    let body: serde_json::Value = resp.json().await.unwrap();
    // Same coordinates → same registered version row.
    assert_eq!(body["version"]["id"].as_str(), Some(v1.to_string().as_str()));
}

/// Full version CRUD: download → get → set-default → delete.
#[tokio::test]
async fn version_crud_lifecycle() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    // GET one
    let get = client
        .get(mock.server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    // set-default (download helper already did it; idempotent re-set)
    let set_default = client
        .post(mock.server.api_url(&format!("/local-runtime/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(set_default.status(), StatusCode::OK);

    // delete (with binary removal)
    let del = client
        .delete(mock.server.api_url(&format!(
            "/local-runtime/versions/{version_id}?remove_binary=true"
        )))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    // gone
    let get2 = client
        .get(mock.server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get2.status(), StatusCode::NOT_FOUND);
}

/// `check-updates` diffs upstream releases against what's installed and
/// flags the build-pending case (tag exists, no binary asset for this host).
#[tokio::test]
async fn check_updates_reports_diff_and_pending_builds() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    let fetch = || async {
        let resp = client
            .get(mock.server.api_url("/local-runtime/versions/llamacpp/check-updates"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        resp.json::<serde_json::Value>().await.unwrap()
    };
    let find = |body: &serde_json::Value, want: &str| -> serde_json::Value {
        body["versions"]
            .as_array()
            .expect("versions array")
            .iter()
            .find(|e| e["version"].as_str() == Some(want))
            .unwrap_or_else(|| panic!("version {want} missing from {body}"))
            .clone()
    };

    // Before installing anything.
    let body = fetch().await;
    assert_eq!(body["engine"].as_str(), Some("llamacpp"));
    assert_eq!(body["platform"].as_str(), Some(mock.platform.as_str()));
    assert_eq!(body["arch"].as_str(), Some(mock.arch.as_str()));

    // TEST_VERSION ships the host cpu asset → ready, not yet installed.
    let test_v = find(&body, mock_release::TEST_VERSION);
    assert_eq!(test_v["binary_ready"].as_bool(), Some(true));
    assert_eq!(test_v["installed"].as_bool(), Some(false));
    assert!(
        test_v["available_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str() == Some("cpu")),
        "expected cpu in available_backends: {test_v}"
    );

    // PENDING_VERSION has no asset → surfaced but not installable.
    let pending = find(&body, mock_release::PENDING_VERSION);
    assert_eq!(pending["binary_ready"].as_bool(), Some(false));
    assert_eq!(pending["installed"].as_bool(), Some(false));
    assert!(pending["available_backends"].as_array().unwrap().is_empty());

    // Install TEST_VERSION, then re-check → now flagged installed.
    lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let body2 = fetch().await;
    let test_v2 = find(&body2, mock_release::TEST_VERSION);
    assert_eq!(test_v2["installed"].as_bool(), Some(true), "should be installed after download: {test_v2}");
    assert!(
        test_v2["installed_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str() == Some("cpu")),
        "expected cpu in installed_backends: {test_v2}"
    );
}

/// Version endpoints reject callers lacking the dedicated
/// `versions_read` / `create` / `delete` permissions (02-permissions F-10
/// split: `llm_local_runtime::read` alone is NOT enough).
#[tokio::test]
async fn version_endpoints_require_permissions() {
    let server = TestServer::start().await;
    // Has instance-read but none of the version permissions.
    let user =
        create_user_with_only_permissions(&server, "reader", &["llm_local_runtime::read"]).await;
    let client = reqwest::Client::new();

    let list = client
        .get(server.api_url("/local-runtime/versions"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::FORBIDDEN, "list needs versions_read");

    let download = client
        .post(server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "engine": "llamacpp", "version": "v0.0.0-test",
            "platform": "linux", "arch": "x86_64", "backend": "cpu"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::FORBIDDEN, "download needs create");
}
