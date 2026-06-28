//! POST /projects/{id}/duplicate behavior.

use reqwest::StatusCode;
use serde_json::{Value, json};

use super::helpers;

#[tokio::test]
async fn clones_metadata_and_files_not_conversations() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({
            "name": "Original",
            "description": "Some desc",
            "instructions": "Speak in haiku.",
        }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    let file = helpers::upload_file(&server, &user, "a.txt", "x").await;
    let fid = file["id"].as_str().unwrap();
    reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/files", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "file_id": fid }))
        .send()
        .await
        .unwrap();

    // Add a conversation to the original — duplicate must NOT copy it.
    let _ = helpers::create_project_conversation(&server, &user, pid).await;

    let dup_resp = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/duplicate", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        dup_resp.status(),
        StatusCode::CREATED,
        "duplicate failed: {}",
        dup_resp.text().await.unwrap_or_default()
    );
    let copy: Value = dup_resp.json().await.unwrap();
    assert_eq!(copy["name"], "Original (copy)");
    assert_eq!(copy["description"], "Some desc");
    assert_eq!(copy["instructions"], "Speak in haiku.");
    assert_ne!(copy["id"], project["id"], "copy must have its own id");

    let copy_id = copy["id"].as_str().unwrap();

    // Files copied (same file ID, new project_files row).
    let files: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}/files", copy_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(files["total"], 1);
    assert_eq!(files["files"][0]["id"], fid);

    // Conversations NOT copied.
    let convs: Value = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}/conversations", copy_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(convs.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn name_collision_appends_copy_n_suffix() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "Foo").await;
    let pid = p["id"].as_str().unwrap();

    let first: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/duplicate", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["name"], "Foo (copy)");

    let second: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/duplicate", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["name"], "Foo (copy 2)");

    let third: Value = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/duplicate", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(third["name"], "Foo (copy 3)");
}

#[tokio::test]
async fn cannot_duplicate_other_users_project() {
    let server = crate::common::TestServer::start().await;
    let user_a = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let user_b = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let p_a = helpers::create_project(&server, &user_a, "Alice's").await;
    let pid_a = p_a["id"].as_str().unwrap();

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/projects/{}/duplicate", pid_a)))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Concurrency: the duplicate handler takes a FOR UPDATE lock to compute the
/// "(copy N)" suffix race-safely. Several simultaneous duplicates of the same
/// project must each get a DISTINCT name (no two racing requests collide on the
/// same suffix). The existing suffix test only duplicates sequentially.
#[tokio::test]
async fn concurrent_duplicates_get_distinct_suffixes() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "concurrent_dup",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "Foo").await;
    let pid = p["id"].as_str().unwrap().to_string();

    // Fire N duplicates concurrently.
    let mut handles = Vec::new();
    for _ in 0..4 {
        let url = server.api_url(&format!("/projects/{}/duplicate", pid));
        let token = user.token.clone();
        handles.push(tokio::spawn(async move {
            let resp = reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
            let body: Value = resp.json().await.unwrap();
            body["name"].as_str().unwrap().to_string()
        }));
    }

    let mut names = Vec::new();
    for h in handles {
        names.push(h.await.unwrap());
    }

    // All names must be unique — the FOR UPDATE lock serializes suffix
    // computation, so no two concurrent duplicates land on the same name.
    let unique: std::collections::HashSet<&String> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "concurrent duplicates must get distinct names, got: {names:?}"
    );
}
