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
        &["files::upload", "files::preview"],
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

// TEST-2 (ITEM-1): owner-scoped (cross-user → 404) + perm-gated (no
// files::preview → 403).
#[tokio::test]
async fn test_get_raw_cross_user_404_and_perm_gate_403() {
    let server = crate::common::TestServer::start().await;

    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "owner",
        &["files::upload", "files::preview"],
    )
    .await;
    // A second user WITH files::preview but who does not own the file → 404.
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "other",
        &["files::upload", "files::preview"],
    )
    .await;
    // A third user WITHOUT files::preview → 403 even for their intent.
    let unprivileged = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "unprivileged",
        &["files::upload"],
    )
    .await;

    let bytes = test_pdf_bytes();
    let file_id = upload_pdf(&server, &owner.token, &bytes).await;

    // Cross-user: owner-scope hides the file as a 404.
    let cross = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}/raw", file_id)))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), StatusCode::NOT_FOUND);

    // Missing files::preview → 403 (permission gate runs before ownership).
    let forbidden = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}/raw", file_id)))
        .header("Authorization", format!("Bearer {}", unprivileged.token))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}
