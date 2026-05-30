//! Tier 2 — models-by-engine-version interface: the usage listing, the
//! same-engine version swap, and the delete guard's "empty → deletable"
//! workflow. Uses the mock release (one downloadable version) + a directly
//! seeded second version, so no second real download is needed.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use super::mock_release::{self, MockReleaseServer};
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Insert a runtime version row directly (a second/installed version the mock
/// can't serve). The binary path is a non-existent stub — these tests never
/// start an instance on the seeded version.
async fn seed_version(
    pool: &PgPool,
    engine: &str,
    version: &str,
    platform: &str,
    arch: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO llm_runtime_versions
            (id, engine, version, platform, arch, backend, binary_path, is_system_default)
         VALUES ($1, $2, $3, $4, $5, 'cpu', '/tmp/ziee-seeded-noexist', FALSE)",
    )
    .bind(id)
    .bind(engine)
    .bind(version)
    .bind(platform)
    .bind(arch)
    .execute(pool)
    .await
    .expect("seed runtime version");
    id
}

async fn fetch_usage(server: &TestServer, token: &str, engine: &str) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/local-runtime/version-usage?engine={engine}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "usage should 200");
    resp.json().await.unwrap()
}

async fn swap(
    server: &TestServer,
    token: &str,
    model_id: Uuid,
    version_id: Uuid,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/models/{model_id}/runtime-version")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "version_id": version_id.to_string() }))
        .send()
        .await
        .expect("swap version")
}

async fn set_default(server: &TestServer, token: &str, version_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/local-runtime/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("set default")
}

async fn delete_version(server: &TestServer, token: &str, version_id: Uuid) -> reqwest::Response {
    reqwest::Client::new()
        .delete(server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("delete version")
}

fn version_entry(usage: &serde_json::Value, version_id: Uuid) -> Option<serde_json::Value> {
    usage["versions"]
        .as_array()?
        .iter()
        .find(|e| e["version"]["id"].as_str() == Some(version_id.to_string().as_str()))
        .cloned()
}

fn model_in(entry: &serde_json::Value, model_id: Uuid) -> Option<serde_json::Value> {
    entry["models"]
        .as_array()?
        .iter()
        .find(|m| m["id"].as_str() == Some(model_id.to_string().as_str()))
        .cloned()
}

/// An unpinned model resolves to the system default and appears under it.
#[tokio::test]
async fn usage_lists_models_under_effective_version() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let v1 = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _t, _p) =
        lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    let model_id = lrt::make_startable_model(
        &mock.server,
        &admin.token,
        &pool,
        provider_id,
        "usage-m1",
        v1,
        "/tmp/ziee-usage.gguf",
    )
    .await;

    let usage = fetch_usage(&mock.server, &admin.token, "llamacpp").await;
    let entry = version_entry(&usage, v1).expect("v1 entry present");
    let m = model_in(&entry, model_id).expect("model resolves to v1 (system default)");
    assert_eq!(
        m["pinned"].as_bool(),
        Some(false),
        "unpinned model inherits the default, not pinned"
    );
    assert_eq!(m["running"].as_bool(), Some(false));
}

/// Swap repins the model to another same-engine version; swapping to a
/// different engine is rejected.
#[tokio::test]
async fn swap_repins_model_and_rejects_engine_mismatch() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let v1 = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _t, _p) =
        lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    let model_id = lrt::make_startable_model(
        &mock.server,
        &admin.token,
        &pool,
        provider_id,
        "swap-m1",
        v1,
        "/tmp/ziee-swap.gguf",
    )
    .await;

    let v2 = seed_version(&pool, "llamacpp", "v0.0.0-test-2", &mock.platform, &mock.arch).await;

    // Swap onto v2 (model not running → no restart).
    let resp = swap(&mock.server, &admin.token, model_id, v2).await;
    assert_eq!(resp.status(), StatusCode::OK, "same-engine swap should 200");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["restarted"].as_bool(), Some(false), "not running → not restarted");

    let usage = fetch_usage(&mock.server, &admin.token, "llamacpp").await;
    let entry2 = version_entry(&usage, v2).expect("v2 entry");
    let m = model_in(&entry2, model_id).expect("model now under v2");
    assert_eq!(m["pinned"].as_bool(), Some(true), "swap pins explicitly");
    if let Some(e1) = version_entry(&usage, v1) {
        assert!(model_in(&e1, model_id).is_none(), "model no longer under v1");
    }

    // A different engine must be refused.
    let v3 = seed_version(&pool, "mistralrs", "v0.0.0-mrs", &mock.platform, &mock.arch).await;
    let resp = swap(&mock.server, &admin.token, model_id, v3).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "cannot change engine via swap");
    let text = resp.text().await.unwrap();
    assert!(text.contains("ENGINE_MISMATCH"), "mismatch reason: {text}");
}

/// A version backing models / being the default cannot be deleted; after all
/// models are swapped away and it is no longer the default, it can.
#[tokio::test]
async fn version_delete_blocked_until_models_swapped_away() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let v1 = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let pool = lrt::test_pool(&mock.server).await;
    let (provider_id, _t, _p) =
        lrt::create_local_provider_with_token(&mock.server, &admin.token).await;
    let model_id = lrt::make_startable_model(
        &mock.server,
        &admin.token,
        &pool,
        provider_id,
        "del-m1",
        v1,
        "/tmp/ziee-del.gguf",
    )
    .await;

    // v1 is the system default → refused.
    let resp = delete_version(&mock.server, &admin.token, v1).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT, "default version delete refused");
    let text = resp.text().await.unwrap();
    assert!(text.contains("VERSION_IN_USE"), "409 reason: {text}");

    // Migrate everything off v1: new default + repin the model.
    let v2 = seed_version(&pool, "llamacpp", "v0.0.0-test-2", &mock.platform, &mock.arch).await;
    assert_eq!(set_default(&mock.server, &admin.token, v2).await.status(), StatusCode::OK);
    assert_eq!(swap(&mock.server, &admin.token, model_id, v2).await.status(), StatusCode::OK);

    // v1 now: not default, no pins, no running instance → deletable.
    let resp = delete_version(&mock.server, &admin.token, v1).await;
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "emptied, non-default version should delete"
    );
}
