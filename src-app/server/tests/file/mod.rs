use crate::common::test_helpers;
use reqwest::multipart;
use std::path::PathBuf;

// File module comprehensive integration tests

// File's chat-bridge tests — moved out of tests/chat/ as part of the
// chat→file bridge extraction. The tests still rely on
// `crate::chat::helpers::*` for model fixtures + SSE parsing
// (`pub(crate)` so cross-module reuse works; same pattern used by
// `tests/project/injection_test.rs`).
mod file_attachments_test;
mod file_attachments_real_providers_test;
mod pdf_raw_test;
mod provider_routing_integration_test;

// Get the path to test data
fn test_data_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/file/test_data")
        .join(filename)
}

// ============================================================================
// File Upload Tests - Basic Functionality
// ============================================================================

#[tokio::test]
async fn test_upload_text_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.txt");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "File upload should succeed");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("id").is_some(), "Should return file ID");
    assert_eq!(body["filename"], "test.txt");
    assert_eq!(body["mime_type"], "text/plain");
    assert!(body["file_size"].as_i64().unwrap() > 0);
    assert_eq!(body["text_page_count"].as_i64().unwrap(), 1, "Text file should have 1 page");
}

#[tokio::test]
async fn test_upload_multiline_text() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("multiline.txt");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("multiline.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "multiline.txt");
    assert_eq!(body["text_page_count"].as_i64().unwrap(), 1);
}

#[tokio::test]
async fn test_upload_csv_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.csv");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.csv")
                .mime_str("text/csv")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.csv");
    assert_eq!(body["mime_type"], "text/csv");
}

#[tokio::test]
async fn test_upload_json_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.json");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.json")
                .mime_str("application/json")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.json");
    assert_eq!(body["mime_type"], "application/json");
}

#[tokio::test]
async fn test_upload_pdf_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.pdf");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.pdf")
                .mime_str("application/pdf")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.pdf");
    assert_eq!(body["mime_type"], "application/pdf");

    // PDF should have previews and text extracted
    let preview_count = body["preview_page_count"].as_i64().unwrap_or(0);
    assert!(preview_count >= 1, "PDF should have at least 1 preview page");

    let has_thumbnail = body["has_thumbnail"].as_bool().unwrap_or(false);
    assert!(has_thumbnail, "PDF should have thumbnail");
}

#[tokio::test]
async fn test_upload_multipage_pdf() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("multipage.pdf");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("multipage.pdf")
                .mime_str("application/pdf")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "multipage.pdf");

    let preview_count = body["preview_page_count"].as_i64().unwrap_or(0);
    assert!(preview_count > 1, "Multi-page PDF should have multiple preview pages");

    let text_page_count = body["text_page_count"].as_i64().unwrap_or(0);
    assert!(text_page_count > 0, "PDF should have text extracted");
}

#[tokio::test]
async fn test_upload_image_jpeg() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.jpg");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.jpg");
    assert_eq!(body["mime_type"], "image/jpeg");

    // Image should have thumbnail and preview
    let has_thumbnail = body["has_thumbnail"].as_bool().unwrap_or(false);
    assert!(has_thumbnail, "Image should have thumbnail");

    let preview_count = body["preview_page_count"].as_i64().unwrap_or(0);
    assert_eq!(preview_count, 1, "Image should have 1 preview");
}

#[tokio::test]
async fn test_upload_image_png() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.png");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.png")
                .mime_str("image/png")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.png");
    assert_eq!(body["mime_type"], "image/png");

    // Image should have thumbnail and preview
    let has_thumbnail = body["has_thumbnail"].as_bool().unwrap_or(false);
    assert!(has_thumbnail, "PNG image should have thumbnail");

    let preview_count = body["preview_page_count"].as_i64().unwrap_or(0);
    assert_eq!(preview_count, 1, "PNG image should have 1 preview");
}

#[tokio::test]
async fn test_upload_docx_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.docx");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.docx")
                .mime_str("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.docx");
    assert!(body["mime_type"].as_str().unwrap().contains("wordprocessingml"));
}

#[tokio::test]
async fn test_upload_xlsx_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("test.xlsx");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.xlsx")
                .mime_str("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "test.xlsx");
    assert!(body["mime_type"].as_str().unwrap().contains("spreadsheetml"));
}

#[tokio::test]
async fn test_upload_xlsx_multisheet() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    let file_path = test_data_path("multisheet.xlsx");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("multisheet.xlsx")
                .mime_str("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["filename"], "multisheet.xlsx");

    // Multi-sheet spreadsheet should extract each sheet as a page
    let text_page_count = body["text_page_count"].as_i64().unwrap_or(0);
    assert!(text_page_count > 1, "Multi-sheet spreadsheet should have multiple text pages");
}

// ============================================================================
// File Provenance Tests (migration 34 — created_by column)
// ============================================================================

#[tokio::test]
async fn test_upload_writes_created_by_user() {
    // The created_by column is provenance metadata: who put the file into the
    // system. Uploads through the HTTP endpoint are always user-attributed
    // (the handler hardcodes "user"). Other write paths — code-sandbox LLM
    // artifacts ("llm"), MCP-tool outputs ("mcp") — will land in follow-on
    // branches; this test pins the upload-path contract so a future refactor
    // can't silently drop or change the default.
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "provenance_user",
        &["files::upload", "files::read"],
    )
    .await;

    let file_bytes = b"provenance check".to_vec();
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(file_bytes)
            .file_name("provenance.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let upload_resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload request failed");
    assert_eq!(upload_resp.status(), 201);

    let upload_body: serde_json::Value = upload_resp.json().await.unwrap();
    assert_eq!(
        upload_body["created_by"], "user",
        "upload response must report created_by='user'",
    );

    // Verify the column is also persisted (read-back via GET metadata).
    let file_id = upload_body["id"].as_str().expect("upload response missing id");
    let get_resp = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}", file_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("get request failed");
    assert_eq!(get_resp.status(), 200);

    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(
        get_body["created_by"], "user",
        "GET /files/{{id}} must return created_by='user' for an uploaded file",
    );
}

// ============================================================================
// File Upload Tests - Validation & Errors
// ============================================================================

#[tokio::test]
async fn test_upload_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_no_permissions(&server, "no_permission_user")
        .await;

    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require files::upload permission");
}

#[tokio::test]
async fn test_upload_no_auth() {
    let server = crate::common::TestServer::start().await;

    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should require authentication");
}

// Boundary against the CONFIGURABLE per-file cap. Spawns a server with a tiny
// 1 MiB cap so the accept/reject boundary is exercised with KB-to-low-MB bodies
// (no 128 MiB allocation). Proves config.server.max_file_upload_mb flows to the
// boot global → both the route body-limit layer and the handler check.
#[tokio::test]
async fn test_upload_size_boundary_respects_configured_cap() {
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        max_file_upload_mb: Some(1),
        ..Default::default()
    })
    .await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;
    let client = reqwest::Client::new();
    let url = server.api_url("/files/upload");

    // Just under the 1 MiB cap → 201 (a file that the old 50 MB / new-default
    // path would also accept, but here proves the tiny configured cap admits it).
    let under = vec![0u8; 900 * 1024];
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(under)
            .file_name("under.bin")
            .mime_str("application/octet-stream")
            .unwrap(),
    );
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 201, "a file under the configured cap must upload");

    // ~1.5 MiB, over the 1 MiB cap but under the derived body limit (cap + 16 MiB),
    // so the HANDLER rejects it with FILE_TOO_LARGE (400) — not an opaque 413.
    let over = vec![0u8; 1024 * 1024 + 512 * 1024];
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(over)
            .file_name("over.bin")
            .mime_str("application/octet-stream")
            .unwrap(),
    );
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 400, "a file over the configured cap must be rejected");
    let body: serde_json::Value = resp.json().await.expect("json error body");
    assert_eq!(body["error_code"], "FILE_TOO_LARGE", "body: {body}");
    assert!(
        body["error"].as_str().unwrap_or_default().contains(" MB"),
        "error message must state the real limit; body: {body}"
    );

    // ~18 MiB — above the DERIVED route body limit (cap 1 MiB + 16 MiB slack =
    // 17 MiB), so the DefaultBodyLimit layer rejects it BEFORE the handler. This
    // distinguishes the derived-from-cap body limit from the old hardcoded 200 MB
    // const (under which 18 MiB would have reached the handler and returned 400
    // FILE_TOO_LARGE), proving the route layer reads the configured cap.
    //
    // axum answers 413 from the Content-Length check; because it responds before
    // consuming the still-uploading body, the client may instead observe a
    // mid-stream connection reset — BOTH prove the body-limit layer engaged. Only
    // a handler response (201, or 400 FILE_TOO_LARGE) would mean the body limit
    // was NOT derived from the cap, so we reject those and accept a non-timeout
    // transport error.
    let over_body_limit = vec![0u8; 18 * 1024 * 1024];
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(over_body_limit)
            .file_name("huge.bin")
            .mime_str("application/octet-stream")
            .unwrap(),
    );
    match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
    {
        Ok(resp) => assert_eq!(
            resp.status(),
            413,
            "a body over the derived route body limit must be rejected by the body-limit layer (413), not reach the handler"
        ),
        Err(e) => assert!(
            !e.is_timeout(),
            "expected 413 or a mid-stream rejection from the body-limit layer, got a timeout: {e}"
        ),
    }
}

// Scientific/genomics binaries must upload. A gzip-framed `.rds` sniffs as
// application/gzip (not HTML), so it must pass the MIME smuggling check and be
// stored with the sniffed gzip MIME — never rejected as MIME_MISMATCH.
#[tokio::test]
async fn test_upload_gzip_rds_binary_accepted() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    // gzip magic (0x1f 0x8b) + deflate method + header, then payload.
    let mut rds = vec![0x1f_u8, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00];
    rds.extend_from_slice(&[0u8; 4096]);
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(rds)
            .file_name("GSE77343.rds")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let url = server.api_url("/files/upload");
    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 201, "gzip-framed .rds must upload (no MIME_MISMATCH)");
    let body: serde_json::Value = resp.json().await.expect("json body");
    assert_eq!(
        body["mime_type"], "application/gzip",
        "sniffed gzip MIME should be stored; body: {body}"
    );
}

// ============================================================================
// File Listing Tests
// ============================================================================

#[tokio::test]
async fn test_list_files() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload a test file first
    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    // List files
    let list_url = server.api_url("/files");
    let response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("files").is_some(), "Should have files array");
    assert!(body.get("total").is_some(), "Should have total count");
    assert!(body.get("page").is_some(), "Should have page number");
    assert!(body.get("per_page").is_some(), "Should have per_page value");

    let files = body["files"].as_array().unwrap();
    assert!(!files.is_empty(), "Should have at least one file");
}

#[tokio::test]
async fn test_list_files_pagination() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload multiple files
    for i in 1..=3 {
        let file_bytes = format!("test content {}", i).into_bytes();
        let form = multipart::Form::new()
            .part(
                "file",
                multipart::Part::bytes(file_bytes)
                    .file_name(format!("test_{}.txt", i))
                    .mime_str("text/plain")
                    .unwrap(),
            );

        let upload_url = server.api_url("/files/upload");
        reqwest::Client::new()
            .post(&upload_url)
            .header("Authorization", format!("Bearer {}", user.token))
            .multipart(form)
            .send()
            .await
            .expect("Upload failed");
    }

    // Test pagination - page 1, 2 items per page
    let list_url = server.api_url("/files?page=1&per_page=2");
    let response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let files = body["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "Should return 2 files on page 1");
    assert_eq!(body["page"].as_i64().unwrap(), 1);
    assert_eq!(body["per_page"].as_i64().unwrap(), 2);
}

#[tokio::test]
async fn test_list_files_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_no_permissions(&server, "no_permission_user")
    .await;

    let url = server.api_url("/files");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require files::read permission");
}

#[tokio::test]
async fn test_list_files_isolation() {
    let server = crate::common::TestServer::start().await;

    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["files::upload", "files::read"],
    )
    .await;

    let user2 = test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["files::upload", "files::read"],
    )
    .await;

    // User 1 uploads a file
    let file_bytes = b"user1 content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("user1.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    // User 2 should only see their own files (none)
    let list_url = server.api_url("/files");
    let response = reqwest::Client::new()
        .get(&list_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let files = body["files"].as_array().unwrap();
    assert_eq!(files.len(), 0, "User 2 should see no files");
}

// ============================================================================
// File Retrieval Tests
// ============================================================================

#[tokio::test]
async fn test_get_file_metadata() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload a file
    let file_bytes = b"test content for metadata";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("metadata_test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Get file metadata
    let get_url = server.api_url(&format!("/files/{}", file_id));
    let response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["id"], file_id);
    assert_eq!(body["filename"], "metadata_test.txt");
    assert_eq!(body["mime_type"], "text/plain");
    assert_eq!(body["file_size"], file_bytes.len() as i64);
}

#[tokio::test]
async fn test_get_file_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::read"],
    )
    .await;

    let fake_id = "00000000-0000-0000-0000-000000000000";
    let url = server.api_url(&format!("/files/{}", fake_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404, "Should return 404 for non-existent file");
}

#[tokio::test]
async fn test_get_file_wrong_user() {
    let server = crate::common::TestServer::start().await;

    let user1 = test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["files::upload"],
    )
    .await;

    let user2 = test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["files::read"],
    )
    .await;

    // User 1 uploads a file
    let file_bytes = b"user1 private content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("private.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user1.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // User 2 tries to get user 1's file
    let get_url = server.api_url(&format!("/files/{}", file_id));
    let response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404, "Should not allow access to other user's files");
}

// ============================================================================
// File Download Tests
// ============================================================================

#[tokio::test]
async fn test_download_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::download"],
    )
    .await;

    // Upload a file first
    let original_content = b"test content for download";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(original_content.to_vec())
                .file_name("download_test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Download the file
    let download_url = server.api_url(&format!("/files/{}/download", file_id));
    let response = reqwest::Client::new()
        .get(&download_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Verify headers
    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "text/plain");
    assert!(headers.get("content-disposition").is_some());
    assert!(headers.get("content-length").is_some());

    // Verify content
    let downloaded_content = response.bytes().await.unwrap();
    assert_eq!(downloaded_content.as_ref(), original_content);
}

#[tokio::test]
async fn test_download_file_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // User with upload permission
    let uploader = test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &["files::upload"],
    )
    .await;

    // User without download permission
    let user = test_helpers::create_user_with_no_permissions(&server, "no_download_user")
    .await;

    // Upload a file
    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", uploader.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Try to download without permission
    let download_url = server.api_url(&format!("/files/{}/download", file_id));
    let response = reqwest::Client::new()
        .get(&download_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require files::download permission");
}

// ============================================================================
// Download Token Tests
// ============================================================================

#[tokio::test]
async fn test_generate_download_token() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::generate_token"],
    )
    .await;

    // Upload a file
    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Generate download token
    let token_url = server.api_url(&format!("/files/{}/download-token", file_id));
    let response = reqwest::Client::new()
        .post(&token_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("token").is_some(), "Should return token");
    assert!(body.get("expires_in").is_some(), "Should return expiry");

    let token = body["token"].as_str().unwrap();
    assert!(!token.is_empty(), "Token should not be empty");
}

#[tokio::test]
async fn test_download_with_token() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::generate_token"],
    )
    .await;

    // Upload a file
    let original_content = b"test content for token download";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(original_content.to_vec())
                .file_name("token_test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Generate download token
    let token_url = server.api_url(&format!("/files/{}/download-token", file_id));
    let token_response = reqwest::Client::new()
        .post(&token_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let token_body: serde_json::Value = token_response.json().await.unwrap();
    let token = token_body["token"].as_str().unwrap();

    // Download with token (no authentication required)
    let download_url = server.api_url(&format!("/files/{}/download-with-token?token={}", file_id, token));
    let response = reqwest::Client::new()
        .get(&download_url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let downloaded_content = response.bytes().await.unwrap();
    assert_eq!(downloaded_content.as_ref(), original_content);
}

#[tokio::test]
async fn test_download_with_invalid_token() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    // Upload a file
    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Try to download with invalid token
    let download_url = server.api_url(&format!("/files/{}/download-with-token?token=invalid-token", file_id));
    let response = reqwest::Client::new()
        .get(&download_url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should reject invalid token");
}

// ==========================================================================
// Text Content Extraction Tests
// ============================================================================

#[tokio::test]
async fn test_get_text_content() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload a text file
    let file_path = test_data_path("test.txt");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Get text content (all pages)
    let text_url = server.api_url(&format!("/files/{}/text", file_id));
    let response = reqwest::Client::new()
        .get(&text_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let text_content = response.text().await.unwrap();
    assert!(text_content.contains("test"), "Should contain extracted text");
    assert!(!text_content.is_empty(), "Text should not be empty");
}

#[tokio::test]
async fn test_get_text_content_specific_page() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload a multi-sheet spreadsheet
    let file_path = test_data_path("multisheet.xlsx");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("multisheet.xlsx")
                .mime_str("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();
    let _text_page_count = upload_body["text_page_count"].as_i64().unwrap();

    // Get specific page
    let text_url = server.api_url(&format!("/files/{}/text?page=1", file_id));
    let response = reqwest::Client::new()
        .get(&text_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let text_content = response.text().await.unwrap();
    assert!(!text_content.is_empty(), "Page 1 text should not be empty");
}

#[tokio::test]
async fn test_get_text_content_invalid_page() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::read"],
    )
    .await;

    // Upload a single-page file
    let file_bytes = b"single page content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("single.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Try to get page 99 (doesn't exist)
    let text_url = server.api_url(&format!("/files/{}/text?page=99", file_id));
    let response = reqwest::Client::new()
        .get(&text_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should return 400 for invalid page number");
}

// ============================================================================
// Preview & Thumbnail Tests
// ============================================================================

#[tokio::test]
async fn test_get_thumbnail() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::preview"],
    )
    .await;

    // Upload an image
    let file_path = test_data_path("test.jpg");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    eprintln!("Upload response: {}", serde_json::to_string_pretty(&upload_body).unwrap());
    let file_id = upload_body["id"].as_str().unwrap();

    // Get thumbnail
    let thumbnail_url = server.api_url(&format!("/files/{}/thumbnail", file_id));
    eprintln!("Requesting thumbnail from: {}", thumbnail_url);
    let response = reqwest::Client::new()
        .get(&thumbnail_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let status = response.status();
    eprintln!("Thumbnail response status: {}", status);

    assert_eq!(status, 200, "Failed to get thumbnail. Status: {}, URL: {}", status, thumbnail_url);

    // Verify headers
    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");
    assert!(headers.get("content-length").is_some());

    // Verify we got image data
    let thumbnail_data = response.bytes().await.unwrap();
    assert!(!thumbnail_data.is_empty(), "Thumbnail should have data");
}

#[tokio::test]
async fn test_get_preview() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::preview"],
    )
    .await;

    // Upload an image
    let file_path = test_data_path("test.jpg");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Get preview (page 1)
    let preview_url = server.api_url(&format!("/files/{}/preview?page=1", file_id));
    let response = reqwest::Client::new()
        .get(&preview_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    // Verify headers
    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");

    // Verify we got image data (should be larger than thumbnail)
    let preview_data = response.bytes().await.unwrap();
    assert!(!preview_data.is_empty(), "Preview should have data");
}

#[tokio::test]
async fn test_get_preview_multipage_pdf() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::preview"],
    )
    .await;

    // Upload a multi-page PDF
    let file_path = test_data_path("multipage.pdf");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("multipage.pdf")
                .mime_str("application/pdf")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();
    let preview_page_count = upload_body["preview_page_count"].as_i64().unwrap();

    // Get preview of page 1
    let preview_url_1 = server.api_url(&format!("/files/{}/preview?page=1", file_id));
    let response_1 = reqwest::Client::new()
        .get(&preview_url_1)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response_1.status(), 200);

    // If there's a second page, get it
    if preview_page_count > 1 {
        let preview_url_2 = server.api_url(&format!("/files/{}/preview?page=2", file_id));
        let response_2 = reqwest::Client::new()
            .get(&preview_url_2)
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response_2.status(), 200);

        // Verify page 1 and page 2 are different
        let data_1 = response_1.bytes().await.unwrap();
        let data_2 = response_2.bytes().await.unwrap();
        assert_ne!(data_1, data_2, "Different pages should have different content");
    }
}

// ============================================================================
// File Deletion Tests
// ============================================================================

#[tokio::test]
async fn test_delete_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::delete"],
    )
    .await;

    // Upload a file first
    let file_bytes = b"test content for deletion";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("delete_test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Delete the file
    let delete_url = server.api_url(&format!("/files/{}", file_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204, "File deletion should succeed");
}

#[tokio::test]
async fn test_delete_file_requires_permission() {
    let server = crate::common::TestServer::start().await;

    let uploader = test_helpers::create_user_with_permissions(
        &server,
        "uploader",
        &["files::upload"],
    )
    .await;

    let user = test_helpers::create_user_with_no_permissions(&server, "no_delete_user")
    .await;

    // Upload a file
    let file_bytes = b"test content";
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes.to_vec())
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", uploader.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Try to delete without permission
    let delete_url = server.api_url(&format!("/files/{}", file_id));
    let response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Should require files::delete permission");
}

#[tokio::test]
async fn test_delete_file_and_verify_cleanup() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload", "files::delete", "files::read"],
    )
    .await;

    // Upload a file with previews
    let file_path = test_data_path("test.jpg");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("cleanup_test.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );

    let upload_url = server.api_url("/files/upload");
    let upload_response = reqwest::Client::new()
        .post(&upload_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Upload failed");

    let upload_body: serde_json::Value = upload_response.json().await.unwrap();
    let file_id = upload_body["id"].as_str().unwrap();

    // Delete the file
    let delete_url = server.api_url(&format!("/files/{}", file_id));
    let delete_response = reqwest::Client::new()
        .delete(&delete_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(delete_response.status(), 204);

    // Verify file is gone from database
    let get_url = server.api_url(&format!("/files/{}", file_id));
    let get_response = reqwest::Client::new()
        .get(&get_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(get_response.status(), 404, "File should be deleted from database");
}

// ============================================================================
// Cache headers — file-content responses carry a private, bounded
// `Cache-Control` so the browser reuses bytes across reloads (fixes laggy
// reloads with many inline files) without surrendering the server-side
// access re-check forever (NOT `immutable`; see FILE_CONTENT_CACHE_CONTROL).
// ============================================================================

#[tokio::test]
async fn test_file_content_responses_have_private_bounded_cache_control() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "cache_hdr_user",
        &["files::upload", "files::download", "files::preview", "files::read"],
    )
    .await;

    // Upload a small text file — processing produces a thumbnail, a preview
    // image, and a text page, so every content endpoint below has something
    // to serve.
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"cache-control check".to_vec())
            .file_name("cache.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let upload_resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload request failed");
    assert_eq!(upload_resp.status(), 201);
    let upload_body: serde_json::Value = upload_resp.json().await.unwrap();
    let file_id = upload_body["id"].as_str().expect("upload response missing id");

    // Every file-content endpoint must carry the private, bounded (NOT
    // immutable) cache header.
    let client = reqwest::Client::new();
    for path in [
        format!("/files/{file_id}/download"),
        format!("/files/{file_id}/preview?page=1"),
        format!("/files/{file_id}/thumbnail"),
        format!("/files/{file_id}/text"),
    ] {
        let resp = client
            .get(server.api_url(&path))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap_or_else(|e| panic!("request to {path} failed: {e}"));
        assert_eq!(resp.status(), 200, "{path} should return 200");

        let cache_control = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            cache_control.contains("private")
                && cache_control.contains("max-age")
                && !cache_control.contains("immutable"),
            "{path} must set a private, bounded (non-immutable) Cache-Control (got {cache_control:?})",
        );
    }
}

// ============================================================================
// Graceful degradation — failed processing still yields a successful upload
// (audit all-f2c43a4939b6: ProcessingResult::default() fallback at
//  file/handlers/upload.rs:165-168)
// ============================================================================

#[tokio::test]
async fn test_upload_unprocessable_pdf_degrades_gracefully() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_degrade_user",
        &["files::upload", "files::download"],
    )
    .await;

    // Valid PDF magic (so MIME sniffing accepts it as application/pdf rather
    // than rejecting it as a MIME mismatch) but a corrupt body the PDF
    // processor cannot turn into pages. The upload must STILL succeed with a
    // degraded (empty) processing result instead of failing the whole request.
    let corrupt_pdf: &[u8] = b"%PDF-1.4\nthis is not a valid PDF body \xff\xfe garbage";

    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(corrupt_pdf.to_vec())
            .file_name("broken.pdf")
            .mime_str("application/pdf")
            .unwrap(),
    );

    let url = server.api_url("/files/upload");
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        201,
        "upload of an unprocessable PDF must still succeed (graceful degradation), not 5xx"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let file_id = body["id"].as_str().expect("response should carry a file id");
    assert_eq!(body["filename"], "broken.pdf");
    assert_eq!(body["mime_type"], "application/pdf");
    assert!(
        body["file_size"].as_i64().unwrap() > 0,
        "the original bytes are still stored despite the processing failure"
    );
    assert_eq!(
        body["text_page_count"].as_i64().unwrap(),
        0,
        "a file that failed processing degrades to zero extracted text pages"
    );

    // The stored original remains downloadable after the degraded upload.
    let dl = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{}/download", file_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("download request failed");
    assert_eq!(
        dl.status(),
        200,
        "the original file must be downloadable after a degraded upload"
    );
    let downloaded = dl.bytes().await.expect("download body");
    assert_eq!(
        downloaded.as_ref(),
        corrupt_pdf,
        "the stored original is returned byte-for-byte"
    );
}

/// File content-dedup through the REAL upload+attach flow: two uploads of the
/// SAME bytes get the same checksum, so when both are attached to a project the
/// `resolve_available_files` resolver (used by the chat replay) collapses them
/// into ONE entry whose `aka` carries the other filename — the production
/// behavior the `dedup_by_checksum` unit tests only cover in isolation.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn dedup_collapses_identical_uploads_in_resolve_available_files() {
    let server = crate::common::TestServer::start().await;
    // This test calls `resolve_available_files` in-process, which reads via the
    // process-global `Repos` factory + file storage; point both at THIS test's
    // DB/store so the in-process resolver sees the HTTP-uploaded files.
    // (Re-init-able factory + `#[serial(repos, file_storage)]`.)
    ziee::init_repositories(sqlx::PgPool::connect(&server.database_url).await.unwrap());
    ziee::init_file_storage(server.data_dir().join("files"));
    let user = test_helpers::create_user_with_permissions(
        &server,
        "dedup_upload",
        &[
            "files::upload",
            "files::read",
            "projects::create",
            "projects::edit",
            "conversations::create",
        ],
    )
    .await;
    let client = reqwest::Client::new();

    // Two uploads, IDENTICAL bytes → identical checksum, different filenames.
    let bytes = b"DEDUP_CONTENT_MARKER identical content for both uploads".to_vec();
    let upload = |name: &'static str| {
        let client = client.clone();
        let token = user.token.clone();
        let url = server.api_url("/files/upload");
        let bytes = bytes.clone();
        async move {
            let form = multipart::Form::new().part(
                "file",
                multipart::Part::bytes(bytes)
                    .file_name(name)
                    .mime_str("text/plain")
                    .unwrap(),
            );
            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .multipart(form)
                .send()
                .await
                .expect("upload");
            assert_eq!(resp.status(), 201, "upload {name} should 201");
            let v: serde_json::Value = resp.json().await.unwrap();
            v["id"].as_str().unwrap().to_string()
        }
    };
    let id_a = upload("alpha.txt").await;
    let id_b = upload("beta.txt").await;
    assert_ne!(id_a, id_b, "two uploads are two distinct file rows");

    // A project, both files attached.
    let project: serde_json::Value = client
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "name": "Dedup Project" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();
    for fid in [&id_a, &id_b] {
        let resp = client
            .post(server.api_url(&format!("/projects/{project_id}/files")))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&serde_json::json!({ "file_id": fid }))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success(), "attach {fid}: {}", resp.status());
    }

    // A conversation in the project so the resolver unions the project files.
    let conv: serde_json::Value = client
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "title": "dedup conv", "project_id": project_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let conv_id = uuid::Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let user_uuid = uuid::Uuid::parse_str(&user.user_id).unwrap();

    // File the conversation under the project via the explicit attach endpoint.
    // (Conversations are created UNFILED; project membership lives in the
    // `project_conversations` join table, set by this call — see the project
    // chat-extension attach handler.)
    let attach = client
        .post(server.api_url(&format!("/projects/{project_id}/conversations/{conv_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(attach.status().is_success(), "attach conversation: {}", attach.status());

    let resolved = ziee::file_available::resolve_available_files(conv_id, user_uuid)
        .await
        .expect("resolve_available_files");

    // The two identical-content uploads collapse to ONE available file, with the
    // second filename surfaced as an alias.
    let our = resolved
        .iter()
        .filter(|f| f.name == "alpha.txt" || f.aka.contains(&"alpha.txt".to_string())
            || f.name == "beta.txt" || f.aka.contains(&"beta.txt".to_string()))
        .collect::<Vec<_>>();
    assert_eq!(
        our.len(),
        1,
        "identical-checksum uploads must dedup to one available file; got {our:?}"
    );
    let f = our[0];
    let names: std::collections::HashSet<&str> =
        std::iter::once(f.name.as_str()).chain(f.aka.iter().map(|s| s.as_str())).collect();
    assert!(
        names.contains("alpha.txt") && names.contains("beta.txt"),
        "both filenames must be reachable on the deduped entry; got name={} aka={:?}",
        f.name, f.aka
    );
}

/// provider_routing dispatch + its defense-in-depth ownership re-validation.
/// (1) Calling the resolver with a DIFFERENT user's id than the file owner is
/// refused (FILE_ACCESS_DENIED) even though callers already gate it.
/// (2) For the owner, a real text file routes to content block(s) (the
/// extracted-text / base64 dispatch arms), not an empty result.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn provider_routing_enforces_ownership_and_routes_text_file() {
    let server = crate::common::TestServer::start().await;
    // `Repos.pool()` is used in-process below; point the global factory at
    // THIS test's DB so it sees the HTTP-uploaded file. (Re-init-able factory
    // + `#[serial(repos)]`.)
    ziee::init_repositories(sqlx::PgPool::connect(&server.database_url).await.unwrap());
    // `process_file_blocks` reads bytes via the process-global
    // `get_file_storage()`; point it at the spawned server's file store so the
    // in-process read sees the HTTP-uploaded file. (`#[serial(file_storage)]`.)
    ziee::init_file_storage(server.data_dir().join("files"));
    let owner = test_helpers::create_user_with_permissions(
        &server,
        "routing_owner",
        &["files::upload", "files::read"],
    )
    .await;
    let other = test_helpers::create_user_with_permissions(
        &server,
        "routing_other",
        &["files::read"],
    )
    .await;

    // Upload a text file as the owner.
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"ROUTING_CONTENT hello provider routing".to_vec())
            .file_name("route.txt")
            .mime_str("text/plain")
            .unwrap(),
    );
    let up: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .multipart(form)
        .send()
        .await
        .expect("upload")
        .json()
        .await
        .unwrap();
    let file_id = uuid::Uuid::parse_str(up["id"].as_str().unwrap()).unwrap();
    let owner_id = uuid::Uuid::parse_str(&owner.user_id).unwrap();
    let other_id = uuid::Uuid::parse_str(&other.user_id).unwrap();

    let pool = ziee::Repos.pool();
    // provider_id is unused on the text dispatch arm — a random id is fine.
    let provider_id = uuid::Uuid::new_v4();

    // (1) Foreign user → forbidden, regardless of provider.
    let denied =
        ziee::file_routing::process_file_blocks(pool, file_id, provider_id, "anthropic", other_id)
            .await;
    let err = denied.expect_err("a foreign user must be refused");
    assert!(
        format!("{err}").to_lowercase().contains("access"),
        "ownership refusal should surface an access error; got: {err}"
    );

    // (2) Owner → routes the text file to at least one content block.
    let blocks =
        ziee::file_routing::process_file_blocks(pool, file_id, provider_id, "anthropic", owner_id)
            .await
            .expect("owner routing should succeed");
    assert!(
        !blocks.is_empty(),
        "a text file must route to at least one content block"
    );
}

/// provider_routing IMAGE arm: an image file routed through a non-native
/// provider (`openai`) takes the base64 image branch of `process_via_base64`
/// (provider_routing.rs:213-221) → a `ContentBlock::Image{ Base64 }`. The
/// existing routing test only covered the TEXT arm; the image/base64 arm was
/// untested.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn provider_routing_image_file_routes_to_base64_image_block() {
    use base64::Engine;

    let server = crate::common::TestServer::start().await;
    // In-process `Repos.pool()` below must target THIS test's DB.
    ziee::init_repositories(sqlx::PgPool::connect(&server.database_url).await.unwrap());
    // `process_file_blocks` reads the uploaded bytes via the process-global
    // `get_file_storage()`; point it at the spawned server's file store so the
    // in-process read sees the HTTP-uploaded image. (Re-init-able global +
    // `#[serial(file_storage)]`.)
    ziee::init_file_storage(server.data_dir().join("files"));
    let owner = test_helpers::create_user_with_permissions(
        &server,
        "routing_img_owner",
        &["files::upload", "files::read"],
    )
    .await;

    // A 1x1 PNG (valid image bytes for the upload pipeline).
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")
        .unwrap();
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(png)
            .file_name("dot.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let up: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .multipart(form)
        .send()
        .await
        .expect("upload")
        .json()
        .await
        .unwrap();
    let file_id = uuid::Uuid::parse_str(up["id"].as_str().unwrap()).unwrap();
    let owner_id = uuid::Uuid::parse_str(&owner.user_id).unwrap();

    let pool = ziee::Repos.pool();
    // "openai" provider_type → an image is NOT routed to a provider Files API
    // (anthropic/gemini only), so it falls to the base64 image branch. The
    // provider_id is unused on that branch — a random id is fine.
    let blocks = ziee::file_routing::process_file_blocks(
        pool,
        file_id,
        uuid::Uuid::new_v4(),
        "openai",
        owner_id,
    )
    .await
    .expect("owner image routing should succeed");

    assert!(
        matches!(blocks.first(), Some(ai_providers::ContentBlock::Image { .. })),
        "an image file must route to a ContentBlock::Image; got: {blocks:?}"
    );
}

/// GET /files/{id}/versions/{version}/preview (`preview_version`): a pinned
/// version's preview image is served, and a non-existent version 404s. The
/// versioning suite covers download_version + text_version but never the
/// preview_version handler.
#[tokio::test]
async fn versioned_preview_endpoint_serves_image_and_404s_unknown_version() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "ver_preview",
        &["files::upload", "files::read", "files::preview"],
    )
    .await;

    // Upload a PDF (the processing pipeline renders preview images for it).
    let bytes = std::fs::read(test_data_path("test.pdf")).expect("read test.pdf");
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(bytes)
            .file_name("doc.pdf")
            .mime_str("application/pdf")
            .unwrap(),
    );
    let up: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("upload")
        .json()
        .await
        .unwrap();
    assert!(up["preview_page_count"].as_i64().unwrap_or(0) >= 1, "PDF must have a preview");
    let file_id = up["id"].as_str().unwrap().to_string();

    // v1 preview is served as a JPEG image.
    let ok = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions/1/preview")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("preview request");
    assert_eq!(ok.status(), 200, "v1 preview should be served");
    assert_eq!(
        ok.headers().get("content-type").and_then(|v| v.to_str().ok()),
        Some("image/jpeg"),
        "preview is a JPEG"
    );
    let img = ok.bytes().await.unwrap();
    assert!(!img.is_empty(), "preview image bytes must be non-empty");

    // A non-existent version → 404 (the version_and_file guard).
    let missing = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/versions/999/preview")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("preview request");
    assert_eq!(missing.status(), 404, "unknown version preview must 404");
}

/// Graceful degradation: a file whose CONTENT processing fails (a .pdf carrying
/// non-PDF garbage bytes → the PDF processor errors) is STILL created with
/// default/empty processing metadata (the `ProcessingResult::default()` fallback
/// in upload.rs), not a 5xx. The upload happy-path tests only cover successful
/// processing.
#[tokio::test]
async fn test_upload_with_failed_processing_still_creates_file() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_degrade_user",
        &["files::upload", "files::read"],
    )
    .await;

    // A .pdf claimed by name + mime, but the bytes are NOT a valid PDF → the
    // PDF processing path errors and falls back to empty results.
    let garbage = b"%PDF-1.7 \x00\x01\x02 not actually a pdf \xff\xfe\xfd junk".to_vec();
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(garbage)
            .file_name("broken.pdf")
            .mime_str("application/pdf")
            .unwrap(),
    );

    let response = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    // The file is CREATED despite the processing failure (graceful, not 5xx).
    assert_eq!(response.status(), 201, "upload must succeed even when processing fails");
    let body: serde_json::Value = response.json().await.expect("parse JSON");
    let file_id = body["id"].as_str().expect("file id").to_string();
    assert_eq!(body["filename"], "broken.pdf");
    // Degraded processing metadata: no thumbnail, no extracted text pages.
    assert_eq!(body["has_thumbnail"], false, "failed processing ⇒ no thumbnail: {body}");
    assert_eq!(body["text_page_count"].as_i64().unwrap_or(-1), 0, "no text pages: {body}");

    // The raw file is still persisted + retrievable (original bytes saved).
    let dl = reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}/download")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(dl.status(), 200, "the saved original must be downloadable");
}

