// ============================================================================
// Memory module integration tests.
//
//   /api/memories             (REST CRUD; MemoryRead / MemoryWrite)
//   /api/memory/settings      (per-user settings; MemoryRead / MemoryWrite)
//   /api/admin/memory-settings (admin; MemoryAdminRead / MemoryAdminManage)
//
// Plan §10 mandatory regressions are exercised here:
//   - cross-user isolation (user A cannot read/write/delete user B's memory)
//   - migration cleanliness (cargo build runs the migration; if it fails
//     compilation will, but we re-assert the tables are populated)
//   - memory-off path (enabled=false → REST works, retrieval no-ops; the
//     latter is asserted in the chat module — outside this file's scope)
//
// Tests assume the build DB has pgvector available (docker-compose uses
// the pgvector/pgvector:pg17 image — gap #1 above).
// ============================================================================

use serde_json::{Value, json};

// ── REST CRUD happy path ────────────────────────────────────────────

#[tokio::test]
async fn test_create_list_get_delete_memory() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_crud",
        &["memory::read", "memory::write"],
    )
    .await;

    // Create
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "content": "User prefers dark mode in code editors",
            "kind": "preference",
            "importance": 60,
        }))
        .send()
        .await
        .expect("create failed");
    assert_eq!(res.status(), 201);
    let row: Value = res.json().await.unwrap();
    let id = row["id"].as_str().expect("response should have id").to_string();
    assert_eq!(row["source"], "manual");
    assert_eq!(row["kind"], "preference");

    // List
    let res = reqwest::Client::new()
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let rows: Vec<Value> = res.json().await.unwrap();
    assert!(rows.iter().any(|r| r["id"] == id));

    // Get
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Delete
    let res = reqwest::Client::new()
        .delete(server.api_url(&format!("/memories/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // Get after delete → 404
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ── Validation ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_memory_rejects_empty_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_empty",
        &["memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": "  " }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_create_memory_rejects_oversize_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_huge",
        &["memory::write"],
    )
    .await;
    let huge = "x".repeat(5_000);
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": huge }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_create_memory_rejects_unknown_kind() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_kind",
        &["memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": "hi", "kind": "bogus" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

// ── Permission gates ────────────────────────────────────────────────

#[tokio::test]
async fn test_create_memory_admin_settings_requires_admin_permission() {
    // memory::read + memory::write are granted to ALL Users via
    // migration 51 (so the default user can list + create their own
    // memories). To prove permission gating on memory routes WORKS,
    // we instead try to hit an ADMIN endpoint with a regular-user
    // permission set — that's the genuinely-gated boundary.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_noadmin",
        &["memory::read", "memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    // 403 forbidden (auth succeeded, missing memory::admin::manage).
    assert_eq!(res.status(), 403);
}

// ── Cross-user isolation (Plan §10 mandatory regression) ────────────

#[tokio::test]
async fn test_cross_user_isolation_get() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_bob",
        &["memory::read", "memory::write"],
    )
    .await;

    // Alice creates a memory.
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", alice.token))
        .json(&json!({ "content": "Alice's secret" }))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    let alice_mem_id = row["id"].as_str().unwrap().to_string();

    // Bob tries to GET it → 404 (not 403; we don't leak existence).
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{}", alice_mem_id)))
        .header("Authorization", format!("Bearer {}", bob.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    // Bob LIST does not contain Alice's memory.
    let res = reqwest::Client::new()
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", bob.token))
        .send()
        .await
        .unwrap();
    let bob_rows: Vec<Value> = res.json().await.unwrap();
    assert!(
        bob_rows.iter().all(|r| r["id"] != alice_mem_id),
        "user B must not see user A's memories in LIST"
    );
}

#[tokio::test]
async fn test_cross_user_isolation_patch_delete() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_alice2",
        &["memory::read", "memory::write"],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_bob2",
        &["memory::read", "memory::write"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", alice.token))
        .json(&json!({ "content": "Alice's data" }))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    let alice_mem_id = row["id"].as_str().unwrap().to_string();

    // Bob's PATCH attempt → 404.
    let res = reqwest::Client::new()
        .patch(server.api_url(&format!("/memories/{}", alice_mem_id)))
        .header("Authorization", format!("Bearer {}", bob.token))
        .json(&json!({ "content": "hijacked" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    // Bob's DELETE attempt → 404.
    let res = reqwest::Client::new()
        .delete(server.api_url(&format!("/memories/{}", alice_mem_id)))
        .header("Authorization", format!("Bearer {}", bob.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);

    // Alice's memory is intact.
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/memories/{}", alice_mem_id)))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["content"], "Alice's data");
}

// ── Settings ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_user_settings_auto_initializes() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_settings",
        &["memory::read", "memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    // Plan §2: defaults are privacy-first opt-in.
    assert_eq!(row["extraction_enabled"], false);
    assert_eq!(row["retrieval_enabled"], false);
}

#[tokio::test]
async fn test_admin_settings_defaults_to_disabled() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_admin",
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;
    let res = reqwest::Client::new()
        .get(server.api_url("/admin/memory-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], false);
    assert_eq!(row["embedding_dimensions"], 768);
    assert!(row["embedding_model_id"].is_null());
}

// ── Delete-all ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_delete_all_only_affects_caller() {
    let server = crate::common::TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_da_alice",
        &["memory::read", "memory::write"],
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "mem_da_bob",
        &["memory::read", "memory::write"],
    )
    .await;

    // Both users create memories.
    for u in [&alice, &bob] {
        reqwest::Client::new()
            .post(server.api_url("/memories"))
            .header("Authorization", format!("Bearer {}", u.token))
            .json(&json!({ "content": "fact" }))
            .send()
            .await
            .unwrap();
    }

    // Alice deletes-all.
    let res = reqwest::Client::new()
        .delete(server.api_url("/memories/all"))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Bob's still there.
    let res = reqwest::Client::new()
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", bob.token))
        .send()
        .await
        .unwrap();
    let rows: Vec<Value> = res.json().await.unwrap();
    assert_eq!(rows.len(), 1, "bob's memory must survive alice's delete-all");
}
