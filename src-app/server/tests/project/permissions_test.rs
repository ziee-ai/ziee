//! Permission gating on project endpoints.

use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn list_projects_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_project_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn read_with_only_create_permission_still_blocks_list() {
    // Having `projects::create` does not imply `projects::read` —
    // each verb is independent. Closes a common rollout mistake where
    // an admin grants only "create" and assumes "list" follows.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["projects::create"],
    )
    .await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn update_requires_edit_permission() {
    let server = crate::common::TestServer::start().await;
    let creator = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "creator",
        super::helpers::full_project_permissions(),
    )
    .await;
    let project = super::helpers::create_project(&server, &creator, "Foo").await;

    let reader_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "reader",
        &["projects::read"],
    )
    .await;

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!(
            "/projects/{}",
            project["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", reader_only.token))
        .json(&json!({ "name": "Renamed" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_requires_delete_permission() {
    let server = crate::common::TestServer::start().await;
    let user_create_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["projects::create", "projects::read"],
    )
    .await;
    let project = super::helpers::create_project(&server, &user_create_only, "Foo").await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}",
            project["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", user_create_only.token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn duplicate_requires_all_of_a_two_permission_and_tuple() {
    // `POST /projects/{id}/duplicate` is gated by the multi-permission tuple
    // `RequirePermissions<(ProjectsCreate, ProjectsRead)>` — the extractor's
    // AND logic (extractors.rs:130-151) requires the caller to hold BOTH.
    // A PARTIAL grant (only one of the two) must be refused at the HTTP layer
    // with 403, and the full grant must succeed. The other project permission
    // tests each exercise a single-permission endpoint, so this is the only
    // coverage of the AND-combining tuple gate.
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // A fully-permissioned owner creates the project to be duplicated.
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "dup_owner",
        super::helpers::full_project_permissions(),
    )
    .await;
    let project = super::helpers::create_project(&server, &owner, "Dup Source").await;
    let pid = project["id"].as_str().unwrap().to_string();
    let dup_url = server.api_url(&format!("/projects/{}/duplicate", pid));

    // Partial grant #1: `projects::read` ONLY (missing `projects::create`).
    // Read access to the row is not enough to clone it — AND logic denies.
    let read_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "dup_read_only",
        &["projects::read"],
    )
    .await;
    let resp = client
        .post(&dup_url)
        .header("Authorization", format!("Bearer {}", read_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "projects::read alone must NOT satisfy the (create, read) AND gate"
    );

    // Partial grant #2: `projects::create` ONLY (missing `projects::read`).
    // The complementary half of the tuple — also denied.
    let create_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "dup_create_only",
        &["projects::create"],
    )
    .await;
    let resp = client
        .post(&dup_url)
        .header("Authorization", format!("Bearer {}", create_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "projects::create alone must NOT satisfy the (create, read) AND gate"
    );

    // Full grant: BOTH permissions present → the AND gate passes. (The owner
    // holds both via full_project_permissions; duplicating one's own project
    // proves the tuple is satisfiable and the 403s above were the gate, not a
    // broken route.)
    let resp = client
        .post(&dup_url)
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "holding BOTH projects::create AND projects::read must satisfy the gate"
    );
async fn attach_conversation_requires_edit_permissions() {
    // POST /projects/{id}/conversations/{conv_id} is gated on
    // (ProjectsEdit, ConversationsEdit). A user holding neither (the perm
    // extractor runs before the handler, so concrete ids aren't needed) is 403.
    let server = crate::common::TestServer::start().await;
    let reader = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "attach_reader",
        &["projects::read"],
    )
    .await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4()
        )))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn detach_conversation_requires_edit_permissions() {
    // DELETE /projects/{id}/conversations/{conv_id} is gated the same way.
    let server = crate::common::TestServer::start().await;
    let reader = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "detach_reader",
        &["projects::read"],
    )
    .await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4()
        )))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
