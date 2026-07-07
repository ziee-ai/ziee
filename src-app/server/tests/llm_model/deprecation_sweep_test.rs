//! TEST-8 (ITEM-8, ITEM-4) + TEST-12 (ITEM-10): the on-demand reconcile route
//! `POST /llm-providers/{id}/refresh-models` flags models the provider no longer
//! lists, emits the dual permission-scoped sync pair, is permission-gated, and
//! is wired into the running server.
//!
//! Uses the debug-only `LLM_DISCOVER_ALLOW_LOOPBACK=1` seam so a 127.0.0.1
//! wiremock can stand in for the provider's `/models` endpoint.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{
    create_user_with_only_permissions, create_user_with_permissions,
};
use crate::common::{TestServer, TestServerOptions};

const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

async fn mock_models(ids: &[&str]) -> MockServer {
    let data: Vec<serde_json::Value> = ids.iter().map(|id| json!({ "id": id })).collect();
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": data })))
        .mount(&upstream)
        .await;
    upstream
}

async fn admin_server() -> (TestServer, String) {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LLM_DISCOVER_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "sweep_admin",
        &[
            "llm_providers::read",
            "llm_providers::create",
            "llm_models::create",
            "llm_models::read",
            "llm_models::edit",
        ],
    )
    .await;
    (server, admin.token)
}

async fn create_openrouter_provider(server: &TestServer, token: &str, base_url: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": format!("or-{}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "openrouter",
            "base_url": base_url,
            "api_key": "test-key",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_model(server: &TestServer, token: &str, provider_id: &str, name: &str) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider_id": provider_id,
            "name": name,
            "display_name": name,
            "engine_type": "mistralrs",
            "file_format": "safetensors",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn refresh(server: &TestServer, token: &str, provider_id: &str) -> (StatusCode, serde_json::Value) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-providers/{provider_id}/refresh-models")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body = resp.json::<serde_json::Value>().await.unwrap_or(json!(null));
    (status, body)
}

fn find<'a>(models: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    models
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == name)
        .unwrap_or_else(|| panic!("model {name} in refreshed list"))
}

/// TEST-8: a model the provider stopped listing is flagged deprecated; a model
/// still listed is not; and the change emits the dual permission-scoped sync pair.
#[tokio::test]
async fn refresh_flags_removed_model_and_emits_sync() {
    let upstream = mock_models(&["model-keep"]).await; // "model-gone" is absent
    let (server, token) = admin_server().await;
    let provider_id = create_openrouter_provider(&server, &token, &upstream.uri()).await;

    let gone_id = create_model(&server, &token, &provider_id, "model-gone").await;
    let _keep_id = create_model(&server, &token, &provider_id, "model-keep").await;

    let mut probe = SyncProbe::open(&server, &token).await;

    let (status, models) = refresh(&server, &token, &provider_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find(&models, "model-gone")["is_deprecated"].as_bool(),
        Some(true),
        "a vanished model must be flagged deprecated"
    );
    assert_eq!(
        find(&models, "model-keep")["is_deprecated"].as_bool(),
        Some(false),
        "a still-listed model must not be flagged"
    );

    // Dual permission-scoped sync pair for the changed model.
    let m = probe.expect_event("llm_model", "update", EVENT_TIMEOUT).await;
    assert_eq!(m.id, gone_id, "llm_model/update must carry the flagged model id");
    let u = probe
        .expect_event("user_llm_provider", "update", EVENT_TIMEOUT)
        .await;
    assert_eq!(u.id, gone_id, "user_llm_provider/update dual-emit");
}

/// TEST-12: the route is wired into the running server and returns the reconciled
/// model list; a no-change refresh returns all models unflagged. Also proves the
/// permission gate (llm_providers::read).
#[tokio::test]
async fn refresh_route_wired_and_permission_gated() {
    let upstream = mock_models(&["m1", "m2"]).await; // both models still listed
    let (server, token) = admin_server().await;
    let provider_id = create_openrouter_provider(&server, &token, &upstream.uri()).await;
    create_model(&server, &token, &provider_id, "m1").await;
    create_model(&server, &token, &provider_id, "m2").await;

    let (status, models) = refresh(&server, &token, &provider_id).await;
    assert_eq!(status, StatusCode::OK, "reconcile route is reachable/wired");
    assert_eq!(models.as_array().unwrap().len(), 2, "returns the model list");
    assert!(
        models
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["is_deprecated"] == json!(false)),
        "no model is flagged when all are still listed"
    );

    // Permission gate: refresh MUTATES, so it requires llm_models::edit.
    // A user with no relevant perms is refused...
    let no_perms =
        create_user_with_only_permissions(&server, "sweep_noperms", &["profile::read"]).await;
    let (forbidden, _) = refresh(&server, &no_perms.token, &provider_id).await;
    assert_eq!(forbidden, StatusCode::FORBIDDEN);

    // ...and — the crux of the read→edit gate — a user who holds only
    // llm_providers::read (which WOULD have sufficed under the old gate) is ALSO
    // refused, locking in that the mutating endpoint needs the write perm.
    let read_only = create_user_with_only_permissions(
        &server,
        "sweep_readonly",
        &["profile::read", "llm_providers::read"],
    )
    .await;
    let (read_forbidden, _) = refresh(&server, &read_only.token, &provider_id).await;
    assert_eq!(
        read_forbidden,
        StatusCode::FORBIDDEN,
        "llm_providers::read alone must NOT authorize the mutating refresh endpoint"
    );
}
