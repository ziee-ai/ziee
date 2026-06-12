//! Real provider E2E tests for file attachments
//!
//! These tests make actual API calls to Anthropic, Gemini, and OpenAI.
//! They use API keys from tests/.env.test
//!
//! Run with:
//! ```bash
//! source tests/.env.test
//! cargo test --test integration_tests chat::file_attachments_real_providers_test -- --nocapture --test-threads=1
//! ```

use std::fs;
use futures_util::StreamExt;

fn get_anthropic_api_key() -> String {
    std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set in tests/.env.test")
}

fn get_openai_api_key() -> String {
    std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set in tests/.env.test")
}

fn get_gemini_api_key() -> String {
    std::env::var("GEMINI_API_KEY")
        .expect("GEMINI_API_KEY must be set in tests/.env.test")
}

// =====================================================
// Anthropic E2E Tests
// =====================================================

#[tokio::test]
async fn test_anthropic_image_vision_e2e() {
    use ai_providers::{Provider, ChatRequest, ChatMessage, ContentBlock, ImageSource, Role};

    let api_key = get_anthropic_api_key();
    let provider = Provider::new("anthropic", &api_key, "https://api.anthropic.com/v1")
        .expect("Failed to create provider");

    // Load test image
    let image_path = concat!(env!("CARGO_MANIFEST_DIR"), "/ai-providers/tests/fixtures/test.jpg");
    let image_data = fs::read(image_path).expect("Failed to read test image");

    eprintln!("📤 Uploading test.jpg ({} bytes) to Anthropic Files API...", image_data.len());

    // Upload to Anthropic Files API
    let upload = ai_providers::FileUpload {
        filename: "test.jpg".to_string(),
        file_data: image_data,
        mime_type: "image/jpeg".to_string(),
    };

    let upload_result = provider.upload_file(upload)
        .await
        .expect("Upload failed")
        .expect("Provider should support file upload");

    eprintln!("✅ Uploaded! File ID: {}", upload_result.provider_file_id);

    // Create message with file reference
    let request = ChatRequest {
        model: "claude-haiku-4-5-20251001".to_string(), // Fast, cheap model
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![
                ContentBlock::Image {
                    source: ImageSource::File {
                        file_id: upload_result.provider_file_id.clone(),
                        media_type: None,
                    },
                },
                ContentBlock::Text {
                    text: "What do you see in this image? Describe it in one sentence.".to_string(),
                },
            ],
        }],
        max_tokens: Some(200),
        ..Default::default()
    };

    eprintln!("🤖 Sending message to Claude with image reference...");

    // Stream response
    let mut stream = provider.chat_stream(request)
        .await
        .expect("Chat stream failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        let chunk = result.expect("Stream error");
        for delta in chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                response_text.push_str(&delta);
                eprint!("{}", delta); // Print as it streams
            }
        }
    }

    eprintln!("\n\n✅ Response received!");
    eprintln!("Response: {}", response_text);

    // Verify response
    assert!(!response_text.is_empty(), "Should receive non-empty response");
    assert!(response_text.len() > 10, "Response should be substantial");

    // Cleanup
    eprintln!("🧹 Deleting file from Anthropic...");
    provider.delete_file(&upload_result.provider_file_id)
        .await
        .expect("Failed to delete file");

    eprintln!("✅ Test complete!");
}

#[tokio::test]
async fn test_anthropic_pdf_document_qa_e2e() {
    use ai_providers::{Provider, ChatRequest, ChatMessage, ContentBlock, DocumentSource, Role};

    let api_key = get_anthropic_api_key();
    let provider = Provider::new("anthropic", &api_key, "https://api.anthropic.com/v1")
        .expect("Failed to create provider");

    // Load test PDF
    let pdf_path = concat!(env!("CARGO_MANIFEST_DIR"), "/ai-providers/tests/fixtures/test.pdf");
    let pdf_data = fs::read(pdf_path).expect("Failed to read test PDF");

    eprintln!("📤 Uploading test.pdf ({} bytes) to Anthropic Files API...", pdf_data.len());

    // Upload to Anthropic Files API
    let upload = ai_providers::FileUpload {
        filename: "test.pdf".to_string(),
        file_data: pdf_data,
        mime_type: "application/pdf".to_string(),
    };

    let upload_result = provider.upload_file(upload)
        .await
        .expect("Upload failed")
        .expect("Provider should support file upload");

    eprintln!("✅ Uploaded! File ID: {}", upload_result.provider_file_id);

    // Create message with document reference
    let request = ChatRequest {
        model: "claude-haiku-4-5-20251001".to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![
                ContentBlock::Document {
                    source: DocumentSource::File {
                        file_id: upload_result.provider_file_id.clone(),
                        media_type: None,
                    },
                },
                ContentBlock::Text {
                    text: "What is in this document? Summarize briefly.".to_string(),
                },
            ],
        }],
        max_tokens: Some(300),
        ..Default::default()
    };

    eprintln!("🤖 Sending message to Claude with PDF reference...");

    // Stream response
    let mut stream = provider.chat_stream(request)
        .await
        .expect("Chat stream failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        let chunk = result.expect("Stream error");
        for delta in chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                response_text.push_str(&delta);
                eprint!("{}", delta);
            }
        }
    }

    eprintln!("\n\n✅ Response received!");
    eprintln!("Response: {}", response_text);

    // Verify response
    assert!(!response_text.is_empty(), "Should receive non-empty response");
    assert!(response_text.len() > 10, "Response should be substantial");

    // Cleanup
    eprintln!("🧹 Deleting file from Anthropic...");
    provider.delete_file(&upload_result.provider_file_id)
        .await
        .expect("Failed to delete file");

    eprintln!("✅ Test complete!");
}

// =====================================================
// OpenAI E2E Tests
// =====================================================

#[tokio::test]
async fn test_openai_image_base64_vision_e2e() {
    use ai_providers::{Provider, ChatRequest, ChatMessage, ContentBlock, ImageSource, Role};
    use base64::Engine;

    let api_key = get_openai_api_key();
    let provider = Provider::new("openai", &api_key, "https://api.openai.com/v1")
        .expect("Failed to create provider");

    // Load and encode test image
    let image_path = concat!(env!("CARGO_MANIFEST_DIR"), "/ai-providers/tests/fixtures/test.jpg");
    let image_data = fs::read(image_path).expect("Failed to read test image");
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&image_data);

    eprintln!("📤 Encoding test.jpg ({} bytes) as base64...", image_data.len());
    eprintln!("Base64 length: {} bytes", base64_data.len());

    // Create message with base64 image
    let request = ChatRequest {
        model: "gpt-4o-mini".to_string(), // Cheap vision model
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![
                ContentBlock::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/jpeg".to_string(),
                        data: base64_data,
                    },
                },
                ContentBlock::Text {
                    text: "What do you see in this image? Describe it in one sentence.".to_string(),
                },
            ],
        }],
        max_tokens: Some(200),
        ..Default::default()
    };

    eprintln!("🤖 Sending message to GPT-4o-mini with base64 image...");

    // Stream response
    let mut stream = provider.chat_stream(request)
        .await
        .expect("Chat stream failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        let chunk = result.expect("Stream error");
        for delta in chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                response_text.push_str(&delta);
                eprint!("{}", delta);
            }
        }
    }

    eprintln!("\n\n✅ Response received!");
    eprintln!("Response: {}", response_text);

    // Verify response
    assert!(!response_text.is_empty(), "Should receive non-empty response");
    assert!(response_text.len() > 10, "Response should be substantial");

    eprintln!("✅ Test complete!");
}

// =====================================================
// Gemini E2E Tests
// =====================================================

#[tokio::test]
async fn test_gemini_image_vision_e2e() {
    use ai_providers::{Provider, ChatRequest, ChatMessage, ContentBlock, ImageSource, Role};

    let api_key = get_gemini_api_key();
    let provider = Provider::new("gemini", &api_key, "https://generativelanguage.googleapis.com/v1beta")
        .expect("Failed to create provider");

    // Load test image
    let image_path = concat!(env!("CARGO_MANIFEST_DIR"), "/ai-providers/tests/fixtures/test.jpg");
    let image_data = fs::read(image_path).expect("Failed to read test image");

    eprintln!("📤 Uploading test.jpg ({} bytes) to Gemini Files API...", image_data.len());

    // Upload to Gemini Files API
    let upload = ai_providers::FileUpload {
        filename: "test.jpg".to_string(),
        file_data: image_data,
        mime_type: "image/jpeg".to_string(),
    };

    let upload_result = provider.upload_file(upload)
        .await
        .expect("Upload failed")
        .expect("Provider should support file upload");

    eprintln!("✅ Uploaded! File URI: {}", upload_result.provider_file_id);

    // Verify expiration metadata (Gemini files expire in 48 hours)
    if let Some(expires_at) = upload_result.expires_at {
        eprintln!("⏰ File expires at: {}", expires_at);

        let duration = expires_at - chrono::Utc::now();
        let hours = duration.num_hours();
        eprintln!("⏳ Time until expiration: {} hours", hours);
        assert!((47..=49).contains(&hours), "Should expire in ~48 hours");
    } else {
        panic!("Gemini files should have expiration time");
    }

    // Create message with file reference
    let request = ChatRequest {
        model: "gemini-2.5-flash".to_string(), // Fast Gemini model
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![
                ContentBlock::Image {
                    source: ImageSource::File {
                        file_id: upload_result.provider_file_id.clone(),
                        media_type: None,
                    },
                },
                ContentBlock::Text {
                    text: "What do you see in this image? Describe it in one sentence.".to_string(),
                },
            ],
        }],
        max_tokens: Some(200),
        ..Default::default()
    };

    eprintln!("🤖 Sending message to Gemini with image reference...");

    // Stream response
    let mut stream = provider.chat_stream(request)
        .await
        .expect("Chat stream failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        let chunk = result.expect("Stream error");
        for delta in chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                response_text.push_str(&delta);
                eprint!("{}", delta);
            }
        }
    }

    eprintln!("\n\n✅ Response received!");
    eprintln!("Response: {}", response_text);

    // Verify response
    assert!(!response_text.is_empty(), "Should receive non-empty response");
    assert!(response_text.len() > 10, "Response should be substantial");

    // Cleanup
    eprintln!("🧹 Deleting file from Gemini...");
    provider.delete_file(&upload_result.provider_file_id)
        .await
        .expect("Failed to delete file");

    eprintln!("✅ Test complete!");
}

#[tokio::test]
async fn test_gemini_pdf_document_qa_e2e() {
    use ai_providers::{Provider, ChatRequest, ChatMessage, ContentBlock, DocumentSource, Role};

    let api_key = get_gemini_api_key();
    let provider = Provider::new("gemini", &api_key, "https://generativelanguage.googleapis.com/v1beta")
        .expect("Failed to create provider");

    // Load test PDF
    let pdf_path = concat!(env!("CARGO_MANIFEST_DIR"), "/ai-providers/tests/fixtures/test.pdf");
    let pdf_data = fs::read(pdf_path).expect("Failed to read test PDF");

    eprintln!("📤 Uploading test.pdf ({} bytes) to Gemini Files API...", pdf_data.len());

    // Upload to Gemini Files API
    let upload = ai_providers::FileUpload {
        filename: "test.pdf".to_string(),
        file_data: pdf_data,
        mime_type: "application/pdf".to_string(),
    };

    let upload_result = provider.upload_file(upload)
        .await
        .expect("Upload failed")
        .expect("Provider should support file upload");

    eprintln!("✅ Uploaded! File URI: {}", upload_result.provider_file_id);

    // Create message with document reference
    let request = ChatRequest {
        model: "gemini-2.5-flash".to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![
                ContentBlock::Document {
                    source: DocumentSource::File {
                        file_id: upload_result.provider_file_id.clone(),
                        media_type: None,
                    },
                },
                ContentBlock::Text {
                    text: "What is in this document? Summarize briefly.".to_string(),
                },
            ],
        }],
        max_tokens: Some(300),
        ..Default::default()
    };

    eprintln!("🤖 Sending message to Gemini with PDF reference...");

    // Stream response
    let mut stream = provider.chat_stream(request)
        .await
        .expect("Chat stream failed");

    let mut response_text = String::new();

    while let Some(result) = stream.next().await {
        let chunk = result.expect("Stream error");
        for delta in chunk.content {
            if let ai_providers::ContentBlockDelta::TextDelta { delta, .. } = delta {
                response_text.push_str(&delta);
                eprint!("{}", delta);
            }
        }
    }

    eprintln!("\n\n✅ Response received!");
    eprintln!("Response: {}", response_text);

    // Verify response
    assert!(!response_text.is_empty(), "Should receive non-empty response");
    assert!(response_text.len() > 10, "Response should be substantial");

    // Cleanup
    eprintln!("🧹 Deleting file from Gemini...");
    provider.delete_file(&upload_result.provider_file_id)
        .await
        .expect("Failed to delete file");

    eprintln!("✅ Test complete!");
}
