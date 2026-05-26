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
