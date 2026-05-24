// File module comprehensive integration tests
use crate::common::test_helpers;
use reqwest::multipart;
use std::path::PathBuf;

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
        .post(&server.api_url("/files/upload"))
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
        .get(&server.api_url(&format!("/files/{}", file_id)))
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

#[tokio::test]
async fn test_upload_file_too_large() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "file_user",
        &["files::upload"],
    )
    .await;

    // Create a file larger than 100MB limit
    let large_file = vec![0u8; 101 * 1024 * 1024];
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(large_file)
                .file_name("large.bin")
                .mime_str("application/octet-stream")
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

    assert_eq!(response.status(), 400, "Should reject files over 100MB");
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
    assert!(files.len() > 0, "Should have at least one file");
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
    let text_page_count = upload_body["text_page_count"].as_i64().unwrap();

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
    assert!(thumbnail_data.len() > 0, "Thumbnail should have data");
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
    assert!(preview_data.len() > 0, "Preview should have data");
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
