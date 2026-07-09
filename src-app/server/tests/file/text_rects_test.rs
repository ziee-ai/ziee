//! TEST-33 (ITEM-25): the citation text-rects endpoint —
//! `GET /files/{id}/text-rects` — is owner-scoped + FilesRead-gated, and returns
//! a graceful `200 {rects:[]}` (page-level fallback) for a non-PDF file instead
//! of a 500. (The real-PDF geometry path is covered by the geometry_persist test
//! where pdfium is available.)

use serde_json::Value;
use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions, TestUser};
use crate::common::TestServer;

async fn upload_text(server: &TestServer, user: &TestUser, name: &str, body: &str) -> String {
    use reqwest::multipart;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(body.as_bytes().to_vec())
            .file_name(name.to_string())
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send().await.unwrap();
    assert_eq!(resp.status(), 201, "upload: {}", resp.text().await.unwrap_or_default());
    let v: Value = resp.json().await.unwrap();
    v["id"].as_str().unwrap().to_string()
}

fn rects_url(server: &TestServer, fid: &str) -> String {
    server.api_url(&format!("/files/{fid}/text-rects?page=1&start=0&end=10"))
}

#[tokio::test]
async fn test_33_text_rects_non_pdf_fallback_and_gating() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "tr_user", &["*"]).await;
    let client = reqwest::Client::new();

    let fid = upload_text(&server, &user, "notes.txt", "some plain text notes here").await;

    // Non-PDF (no geometry) → 200 with an empty rect set (page-level fallback).
    let resp = client
        .get(rects_url(&server, &fid))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200, "non-PDF text-rects is a 200 fallback, not 500");
    let body: Value = resp.json().await.unwrap();
    assert!(body["rects"].as_array().unwrap().is_empty(), "no geometry → empty rects: {body}");
    assert!(body.get("page_w").is_some() && body.get("page_h").is_some());

    // Foreign file → 404 (get_by_id_and_user owner scope).
    let other = create_user_with_permissions(&server, "tr_other", &["*"]).await;
    let foreign = client
        .get(rects_url(&server, &fid))
        .header("Authorization", format!("Bearer {}", other.token))
        .send().await.unwrap();
    assert_eq!(foreign.status(), 404, "another user's file → 404");

    // No files::read permission → 403.
    let noperm = create_user_with_no_permissions(&server, "tr_noperm").await;
    let denied = client
        .get(rects_url(&server, &fid))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send().await.unwrap();
    assert_eq!(denied.status(), 403, "text-rects gates on files::read");
}
