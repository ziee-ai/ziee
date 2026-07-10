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

// TEST-32 (ITEM-23,24): a real PDF's ingest-time geometry is persisted and
// re-derived for a chunk's cleaned span → non-empty fraction rects. (pdfium is
// available in the test image, as the pdf_raw test relies on it.)
#[tokio::test]
async fn test_32_pdf_geometry_persisted_and_text_rects_derived() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "geom_user", &["*"]).await;
    let client = reqwest::Client::new();

    // upload a real multi-page PDF fixture
    use reqwest::multipart;
    let pdf = include_bytes!("test_data/3_pages.pdf").to_vec();
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(pdf).file_name("3_pages.pdf").mime_str("application/pdf").unwrap(),
    );
    let up = client.post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form).send().await.unwrap();
    assert_eq!(up.status(), 201, "pdf upload: {}", up.text().await.unwrap_or_default());
    let fid = up.json::<Value>().await.unwrap()["id"].as_str().unwrap().to_string();

    // wait until chunks exist, then pull one chunk's (page, char span).
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(2)
        .connect(&server.database_url).await.unwrap();
    let fuid = uuid::Uuid::parse_str(&fid).unwrap();
    let mut row: Option<(i32, i32, i32)> = None;
    for _ in 0..40 {
        row = sqlx::query_as::<_, (i32, i32, i32)>(
            "SELECT page_number, char_start, char_end FROM file_chunks \
             WHERE file_id = $1 AND page_number IS NOT NULL ORDER BY char_start LIMIT 1",
        ).bind(fuid).fetch_optional(&pool).await.unwrap();
        if row.is_some() { break; }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let (page, start, end) = row.expect("a chunk with a page number");

    let resp = client
        .get(server.api_url(&format!("/files/{fid}/text-rects?page={page}&start={start}&end={end}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let rects = body["rects"].as_array().unwrap();
    assert!(!rects.is_empty(), "a real PDF chunk span yields highlight rects: {body}");
    // rects are fraction-normalized to [0,1].
    for r in rects {
        for k in ["x", "y", "w", "h"] {
            let v = r[k].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&v), "rect {k}={v} normalized in [0,1]");
        }
    }
}
