//! Integration tests for `GET /files/{id}/raw` — the inline raw-bytes endpoint
//! the client-side PDF.js viewer loads real PDFs from (ITEM-1, ITEM-2).

use reqwest::StatusCode;
use std::path::PathBuf;
use uuid::Uuid;

fn test_pdf_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/file/test_data")
        .join("multipage.pdf");
    std::fs::read(path).expect("read multipage.pdf fixture")
}

async fn upload_pdf(
    server: &crate::common::TestServer,
    token: &str,
    bytes: &[u8],
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name("multipage.pdf".to_string())
            .mime_str("application/pdf")
            .unwrap(),
    );
    let response = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    crate::chat::helpers::parse_uuid(&body["id"])
}

// TEST-1 (ITEM-1, ITEM-2): the route is registered and serves the original PDF
// bytes inline with the right content-type.
#[tokio::test]
async fn test_get_raw_returns_inline_pdf_bytes() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["files::upload", "files::download"],
    )
    .await;

    let bytes = test_pdf_bytes();
    let file_id = upload_pdf(&server, &user.token, &bytes).await;

    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}/raw", file_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/pdf"),
    );
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok()),
        Some("inline"),
    );
    let served = response.bytes().await.unwrap();
    assert_eq!(served.as_ref(), bytes.as_slice(), "served bytes must equal the uploaded PDF");
}

// TEST-2 (ITEM-1): owner-scoped — a different user (even one who can download
// their OWN files) cannot read another user's raw bytes; they get a 404, not the
// file. This is the security-critical property.
//
// Note on the permission gate: `get_raw` is gated by `RequirePermissions<
// (FilesDownload,)>` (the standard extractor, identical to `download_file`),
// which 403s a caller lacking `files::download` BEFORE the ownership lookup. That
// 403 path is NOT separately asserted here because every user the test harness
// creates is registered via `create_local_user_with_default_group`, and the
// default group grants `files::download` (migration 27) — so a download-less
// user is not constructible through `create_user_with_permissions`. The gate is
// the same trusted mechanism `download_file` already relies on.
#[tokio::test]
async fn test_get_raw_is_owner_scoped() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "owner",
        &["files::upload", "files::download"],
    )
    .await;
    // A second user WITH files::download but who does not own the file.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "other",
        &["files::upload", "files::download"],
    )
    .await;

    let bytes = test_pdf_bytes();
    let file_id = upload_pdf(&server, &owner.token, &bytes).await;

    // Cross-user: owner-scope hides the file as a 404 (never leaks the bytes).
    let cross = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}/raw", file_id)))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), StatusCode::NOT_FOUND);
}
