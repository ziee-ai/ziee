//! Integration tests for the desktop host-mount feature (feature #3).
//!
//! Tier 3 (HTTP): `/api/host-mounts/*` endpoints — auth/permission gating,
//! ownership, validation, round-trips. Tier 2 (DB): the repository's
//! read-through resolution (conversation → project fallback).
//!
//! Served by `ziee-desktop --headless`; mirrors the `remote_access` tests.

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use ziee_desktop::modules::host_mount::models::MountEntry;
use ziee_desktop::modules::host_mount::repository::HostMountRepository;

async fn pool_for(server: &crate::common::TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("connect test DB")
}

async fn insert_conversation(pool: &sqlx::PgPool, owner: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO conversations (id, user_id, title) VALUES ($1, $2, 'test')")
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await
        .expect("insert conversation");
    id
}

async fn insert_project(pool: &sqlx::PgPool, owner: Uuid, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO projects (id, user_id, name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(owner)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert project");
    id
}

// ===================== policy =====================

#[tokio::test]
async fn policy_get_requires_auth() {
    let server = crate::common::TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .get(server.api_url("/host-mounts/policy"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn policy_get_requires_read_permission() {
    let server = crate::common::TestServer::start_desktop().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "hm_noperm").await;
    let res = reqwest::Client::new()
        .get(server.api_url("/host-mounts/policy"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn policy_update_requires_manage() {
    let server = crate::common::TestServer::start_desktop().await;
    // Read-only user may GET but not PUT.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_readonly",
        &["host_mount::read"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/host-mounts/policy"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn policy_roundtrip() {
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_admin",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let client = reqwest::Client::new();

    let put = client
        .put(server.api_url("/host-mounts/policy"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "enabled": true,
            "allowed_prefixes": ["/Users/me/data", "/Volumes"],
            "allow_readwrite": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let get: serde_json::Value = client
        .get(server.api_url("/host-mounts/policy"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get["enabled"], true);
    assert_eq!(get["allow_readwrite"], true);
    assert_eq!(get["allowed_prefixes"][0], "/Users/me/data");
    assert_eq!(get["allowed_prefixes"][1], "/Volumes");
}

// ===================== conversation / project scope =====================

#[tokio::test]
async fn conversation_mounts_owner_only_404() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_owner",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_other",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;

    // Conversation owned by `owner`; `other` must not be able to touch it.
    let conv = insert_conversation(&pool, Uuid::parse_str(&owner.user_id).unwrap()).await;

    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/host-mounts/conversation/{conv}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "mounts": [{ "host_path": "/Users/me/data", "read_only": true }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn conversation_mounts_roundtrip() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_conv",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let conv = insert_conversation(&pool, Uuid::parse_str(&user.user_id).unwrap()).await;
    let client = reqwest::Client::new();

    let put = client
        .put(server.api_url(&format!("/host-mounts/conversation/{conv}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "mounts": [{ "host_path": "/Users/me/runs", "read_only": true }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let get: serde_json::Value = client
        .get(server.api_url(&format!("/host-mounts/conversation/{conv}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get["mounts"][0]["host_path"], "/Users/me/runs");
    assert_eq!(get["mounts"][0]["read_only"], true);
}

#[tokio::test]
async fn project_mounts_roundtrip() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_proj",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let proj = insert_project(&pool, Uuid::parse_str(&user.user_id).unwrap(), "Proj A").await;
    let client = reqwest::Client::new();

    let put = client
        .put(server.api_url(&format!("/host-mounts/project/{proj}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "mounts": [{ "host_path": "/Volumes/ext/genomes", "read_only": true }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let get: serde_json::Value = client
        .get(server.api_url(&format!("/host-mounts/project/{proj}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get["mounts"][0]["host_path"], "/Volumes/ext/genomes");
}

#[tokio::test]
async fn mount_validation_rejects_empty_path() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_val",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let conv = insert_conversation(&pool, Uuid::parse_str(&user.user_id).unwrap()).await;

    let res = reqwest::Client::new()
        .put(server.api_url(&format!("/host-mounts/conversation/{conv}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "mounts": [{ "host_path": "", "read_only": true }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
}

// ===================== Tier 2: read-through resolution =====================

#[tokio::test]
async fn read_through_conversation_falls_back_to_project() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "hm_rt",
        &["host_mount::read", "host_mount::manage"],
    )
    .await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    let proj = insert_project(&pool, uid, "RT Proj").await;
    let conv = insert_conversation(&pool, uid).await;
    sqlx::query("INSERT INTO project_conversations (conversation_id, project_id) VALUES ($1, $2)")
        .bind(conv)
        .bind(proj)
        .execute(&pool)
        .await
        .expect("link conversation to project");

    let repo = HostMountRepository::new(pool.clone());

    // Project has a mount; conversation has none → read-through returns the
    // project's mounts.
    repo.upsert_project(
        proj,
        uid,
        &[MountEntry { host_path: "/data/project".into(), read_only: true }],
    )
    .await
    .unwrap();
    let eff = repo.resolve_effective(conv, uid).await.unwrap();
    assert_eq!(eff.len(), 1, "conversation inherits the project mount");
    assert_eq!(eff[0].host_path, "/data/project");

    // Now the conversation has its own mounts → they OVERRIDE the project's.
    repo.upsert_conversation(
        conv,
        uid,
        &[MountEntry { host_path: "/data/conv".into(), read_only: false }],
    )
    .await
    .unwrap();
    let eff2 = repo.resolve_effective(conv, uid).await.unwrap();
    assert_eq!(eff2.len(), 1, "conversation row overrides project");
    assert_eq!(eff2[0].host_path, "/data/conv");
    assert!(!eff2[0].read_only);
}
