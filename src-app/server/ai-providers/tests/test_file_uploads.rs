//! Integration tests for file upload functionality
//!
//! These tests verify that file uploads work correctly with each provider's Files API.
//! All tests are marked with #[ignore] and must be run manually with API keys.
//!
//! Setup:
//! 1. Source the environment file: `source tests/.env.test`
//! 2. Run tests: `cargo test --test test_file_uploads -- --ignored --nocapture`

use ai_providers::{AnthropicProvider, GeminiProvider, OpenAIProvider, FileUpload, AIProvider};
use std::fs;

/// Test PDF file (small, valid PDF)
const TEST_PDF_PATH: &str = "tests/fixtures/test.pdf";

/// Test image file (small, valid JPEG)
const TEST_IMAGE_PATH: &str = "tests/fixtures/test.jpg";

fn get_test_file(path: &str) -> Vec<u8> {
    fs::read(path).expect(&format!("Failed to read test file: {}", path))
}

#[tokio::test]
#[ignore] // Run manually with: cargo test --test test_file_uploads test_anthropic_upload_pdf -- --ignored --nocapture
async fn test_anthropic_upload_pdf() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set in tests/.env.test");

    let provider = AnthropicProvider;
    let file_data = get_test_file(TEST_PDF_PATH);

    let upload = FileUpload {
        filename: "test.pdf".to_string(),
        file_data,
        mime_type: "application/pdf".to_string(),
    };

    println!("Uploading PDF to Anthropic Files API...");
    let result = provider
        .upload_file(&api_key, "https://api.anthropic.com/v1", upload)
        .await;

    match result {
        Ok(Some(response)) => {
            println!("✅ Upload succeeded!");
            println!("Provider file ID: {}", response.provider_file_id);
            println!("Expires at: {:?}", response.expires_at);
            println!("Metadata: {}", serde_json::to_string_pretty(&response.metadata).unwrap());

            // Verify no expiration for Anthropic
            assert!(response.expires_at.is_none(), "Anthropic files should not expire");
            assert!(!response.provider_file_id.is_empty(), "File ID should not be empty");

            // Test cleanup (delete file)
            println!("\nDeleting uploaded file...");
            let delete_result = provider
                .delete_file(&api_key, "https://api.anthropic.com/v1", &response.provider_file_id)
                .await;

            assert!(delete_result.is_ok(), "File deletion failed: {:?}", delete_result);
            println!("✅ File deleted successfully");
        }
        Ok(None) => {
            panic!("Provider returned None (should support file uploads)");
        }
        Err(e) => {
            panic!("Upload failed: {:?}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_anthropic_upload_image() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set");

    let provider = AnthropicProvider;
    let file_data = get_test_file(TEST_IMAGE_PATH);

    let upload = FileUpload {
        filename: "test.jpg".to_string(),
        file_data,
        mime_type: "image/jpeg".to_string(),
    };

    println!("Uploading JPEG to Anthropic Files API...");
    let result = provider
        .upload_file(&api_key, "https://api.anthropic.com/v1", upload)
        .await;

    match result {
        Ok(Some(response)) => {
            println!("✅ Upload succeeded!");
            println!("Provider file ID: {}", response.provider_file_id);

            // Cleanup
            let _ = provider
                .delete_file(&api_key, "https://api.anthropic.com/v1", &response.provider_file_id)
                .await;
        }
        Ok(None) => panic!("Provider returned None"),
        Err(e) => panic!("Upload failed: {:?}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_upload_pdf() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY not set in tests/.env.test");

    let provider = GeminiProvider;
    let file_data = get_test_file(TEST_PDF_PATH);

    let upload = FileUpload {
        filename: "test.pdf".to_string(),
        file_data,
        mime_type: "application/pdf".to_string(),
    };

    println!("Uploading PDF to Gemini File API...");
    let result = provider
        .upload_file(
            &api_key,
            "https://generativelanguage.googleapis.com/v1beta",
            upload,
        )
        .await;

    match result {
        Ok(Some(response)) => {
            println!("✅ Upload succeeded!");
            println!("Provider file URI: {}", response.provider_file_id);
            println!("Expires at: {:?}", response.expires_at);
            println!("Metadata: {}", serde_json::to_string_pretty(&response.metadata).unwrap());

            // Verify 48-hour expiration for Gemini
            assert!(response.expires_at.is_some(), "Gemini files should have expiration");

            let expires_at = response.expires_at.unwrap();
            let now = chrono::Utc::now();
            let diff = expires_at - now;

            println!("Time until expiration: {} hours", diff.num_hours());

            // Should be approximately 48 hours (allow some margin)
            assert!(diff.num_hours() >= 47 && diff.num_hours() <= 49,
                "Expiration should be ~48 hours, got {} hours", diff.num_hours());

            // Test cleanup
            println!("\nDeleting uploaded file...");
            let delete_result = provider
                .delete_file(
                    &api_key,
                    "https://generativelanguage.googleapis.com/v1beta",
                    &response.provider_file_id,
                )
                .await;

            assert!(delete_result.is_ok(), "File deletion failed: {:?}", delete_result);
            println!("✅ File deleted successfully");
        }
        Ok(None) => {
            panic!("Provider returned None (should support file uploads)");
        }
        Err(e) => {
            panic!("Upload failed: {:?}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_gemini_upload_image() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY not set");

    let provider = GeminiProvider;
    let file_data = get_test_file(TEST_IMAGE_PATH);

    let upload = FileUpload {
        filename: "test.jpg".to_string(),
        file_data,
        mime_type: "image/jpeg".to_string(),
    };

    println!("Uploading JPEG to Gemini File API...");
    let result = provider
        .upload_file(
            &api_key,
            "https://generativelanguage.googleapis.com/v1beta",
            upload,
        )
        .await;

    match result {
        Ok(Some(response)) => {
            println!("✅ Upload succeeded!");
            println!("Provider file URI: {}", response.provider_file_id);

            // Cleanup
            let _ = provider
                .delete_file(
                    &api_key,
                    "https://generativelanguage.googleapis.com/v1beta",
                    &response.provider_file_id,
                )
                .await;
        }
        Ok(None) => panic!("Provider returned None"),
        Err(e) => panic!("Upload failed: {:?}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_openai_supports_document_upload() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY not set");

    let provider = OpenAIProvider;

    // OpenAI supports the Files API for documents (purpose=user_data).
    assert!(provider.supports_file_api(), "OpenAI should support the Files API for documents");

    let file_data = get_test_file(TEST_PDF_PATH);

    let upload = FileUpload {
        filename: "test.pdf".to_string(),
        file_data,
        mime_type: "application/pdf".to_string(),
    };

    println!("Uploading a PDF to OpenAI (should return a file_id)...");
    let result = provider
        .upload_file(&api_key, "https://api.openai.com/v1", upload)
        .await;

    match result {
        Ok(Some(resp)) => {
            assert!(!resp.provider_file_id.is_empty(), "expected a non-empty file_id");
            assert!(resp.expires_at.is_none(), "OpenAI files don't expire");
            println!("✅ Uploaded document, file_id={}", resp.provider_file_id);
        }
        Ok(None) => {
            panic!("OpenAI should support document upload (returned None)");
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }
}

#[test]
fn test_provider_capabilities() {
    let anthropic = AnthropicProvider;
    let gemini = GeminiProvider;
    let openai = OpenAIProvider;

    // Test supports_file_api()
    assert!(anthropic.supports_file_api(), "Anthropic should support Files API");
    assert!(gemini.supports_file_api(), "Gemini should support File API");
    // OpenAI supports the Files API for documents/PDFs (images stay base64 — the
    // server router keeps image file_ids off OpenAI since they're Responses-API
    // only). The provider-level capability flag is therefore true.
    assert!(openai.supports_file_api(), "OpenAI should support the Files API for documents");

    // Test file_expiration()
    assert!(anthropic.file_expiration().is_none(), "Anthropic files should not expire");

    let gemini_expiration = gemini.file_expiration();
    assert!(gemini_expiration.is_some(), "Gemini files should have expiration");
    assert_eq!(gemini_expiration.unwrap().num_hours(), 48, "Gemini files should expire in 48 hours");

    assert!(openai.file_expiration().is_none(), "OpenAI default should be no expiration");
}

#[tokio::test]
#[ignore]
async fn test_large_file_handling() {
    dotenv::from_filename("tests/.env.test").ok();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY not set");

    let provider = AnthropicProvider;

    // Create a 1MB dummy PDF (not a real PDF, just for size testing)
    let large_data = vec![0u8; 1024 * 1024];  // 1 MB

    let upload = FileUpload {
        filename: "large_test.bin".to_string(),
        file_data: large_data,
        mime_type: "application/octet-stream".to_string(),
    };

    println!("Uploading 1MB file...");
    let result = provider
        .upload_file(&api_key, "https://api.anthropic.com/v1", upload)
        .await;

    // This might fail with "Invalid file type" which is expected
    // We're testing that the upload mechanism handles large files
    match result {
        Ok(Some(response)) => {
            println!("✅ Large file upload succeeded");
            // Cleanup
            let _ = provider
                .delete_file(&api_key, "https://api.anthropic.com/v1", &response.provider_file_id)
                .await;
        }
        Ok(None) => {
            panic!("Unexpected None result");
        }
        Err(e) => {
            println!("⚠️  Large file upload failed (may be expected): {:?}", e);
            // This is okay - we're mainly testing that it doesn't crash
        }
    }
}
