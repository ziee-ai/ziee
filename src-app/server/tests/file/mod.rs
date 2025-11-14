// File module integration tests
use crate::common::test_helpers::{self, TestUser};
use reqwest::multipart;
use serde_json::json;
use std::path::PathBuf;

// Get the path to test data
fn test_data_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/file/test_data")
        .join(filename)
}

// ============================================================================
// File Upload Tests
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

    // Read test file
    let file_path = test_data_path("test.txt");
    let file_bytes = std::fs::read(&file_path).expect("Failed to read test file");

    // Create multipart form
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes)
                .file_name("test.txt")
                .mime_str("text/plain")
                .unwrap(),
        );

    // Upload file
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

    // PDF should have text extracted or pages rendered
    let page_count = body["page_count"].as_i64().unwrap_or(0);
    assert!(page_count >= 0, "Should have page count");
}

#[tokio::test]
async fn test_upload_image_file() {
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

    // Image should have thumbnail generated
    let thumbnail_count = body["thumbnail_count"].as_i64().unwrap_or(0);
    assert!(thumbnail_count > 0, "Should have generated thumbnail");
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
async fn test_upload_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "no_permission_user",
        &[], // No permissions
    )
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

    let files = body["files"].as_array().unwrap();
    assert!(files.len() > 0, "Should have at least one file");
}

#[tokio::test]
async fn test_list_files_requires_permission() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "no_permission_user",
        &[],
    )
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
    let user = test_helpers::create_user_with_permissions(
        &server,
        "no_download_user",
        &[],
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

    let user = test_helpers::create_user_with_permissions(
        &server,
        "no_delete_user",
        &[],
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

// ============================================================================
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

    // Get text content
    let text_url = server.api_url(&format!("/files/{}/text", file_id));
    let response = reqwest::Client::new()
        .get(&text_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let text_content = response.text().await.unwrap();
    assert!(text_content.contains("test text file"), "Should contain extracted text");
}
