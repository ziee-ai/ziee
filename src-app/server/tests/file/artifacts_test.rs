//! Integration tests for the artifacts/deliverables backend: user append-version
//! (the user side of co-editing a deliverable) and multi-format file export.

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

async fn upload_text(
    server: &crate::common::TestServer,
    token: &str,
    filename: &str,
    content: &str,
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.as_bytes().to_vec())
            .file_name(filename.to_string())
            .mime_str("text/markdown")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = resp.json().await.unwrap();
    crate::chat::helpers::parse_uuid(&body["id"])
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// TEST-10 (ITEM-1): append-version bumps the head and changes the content.
#[tokio::test]
async fn test_append_version_bumps_head_and_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_user",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &user.token, "report.md", "# Draft\n\nv1 content\n").await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": "# Draft\n\nEDITED content\n" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"].as_i64().unwrap(), 2, "head version bumps to 2");

    let text = client()
        .get(server.api_url(&format!("/files/{fid}/text")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(text.contains("EDITED"), "head text reflects the edit; got: {text}");
}

// TEST-10 (ITEM-1): byte-identical content is a no-op (content-addressed).
#[tokio::test]
async fn test_append_version_noop_on_identical() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_noop",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let content = "# Same\n\nunchanged\n";
    let fid = upload_text(&server, &user.token, "same.md", content).await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": content }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"].as_i64().unwrap(), 1, "identical content does not add a version");
}

// TEST-10 (ITEM-1): another user's file id is not found (ownership-scoped).
#[tokio::test]
async fn test_append_version_cross_user_404() {
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_owner",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_other",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &owner.token, "owned.md", "secret\n").await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "content": "hijack\n" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn export_bytes(
    server: &crate::common::TestServer,
    token: &str,
    fid: Uuid,
    format: &str,
) -> (StatusCode, Vec<u8>) {
    let resp = client()
        .get(server.api_url(&format!("/files/{fid}/export?format={format}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.bytes().await.unwrap().to_vec();
    (status, bytes)
}

// TEST-12/13 (ITEM-3, ITEM-23): export a markdown deliverable to md/docx/pdf/html.
#[tokio::test]
async fn test_file_export_formats() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_export",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &user.token, "doc.md", "# Title\n\nHello **world**.\n").await;

    let (s, md) = export_bytes(&server, &user.token, fid, "md").await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8_lossy(&md).contains("# Title"), "md is the raw source");

    let (s, docx) = export_bytes(&server, &user.token, fid, "docx").await;
    assert_eq!(s, StatusCode::OK);
    assert!(docx.len() > 4 && &docx[..2] == b"PK", "docx is an OOXML zip (PK magic)");

    let (s, pdf) = export_bytes(&server, &user.token, fid, "pdf").await;
    assert_eq!(s, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"), "pdf starts with %PDF");

    let (s, html) = export_bytes(&server, &user.token, fid, "html").await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        String::from_utf8_lossy(&html).to_lowercase().contains("title"),
        "html contains the content"
    );

    let (s, _) = export_bytes(&server, &user.token, fid, "bogus").await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "unsupported format is rejected");
}
