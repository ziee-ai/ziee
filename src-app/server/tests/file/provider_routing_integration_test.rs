//! audit id all-11565533d3a9 — provider-routing edge cases through the REAL
//! integration path (provider_routing.rs:61-103 → process_file_blocks →
//! process_via_base64). The pure `route_decision` branches are unit-tested
//! in-source, but nothing exercised the DB+storage path that actually turns an
//! attached file into the ContentBlock(s) the model receives. Here we attach
//! files to a real chat message against a CAPTURING stub provider and assert
//! what reaches the wire:
//!   - a text/* file → its verbatim body is inlined (Base64 text branch), so
//!     the marker appears in the provider request.
//!   - an unsupported binary → a labeled placeholder, NOT the raw bytes.
//! The stub LLM is the only mocked boundary; routing is the behavior under test.

use serde_json::json;
use uuid::Uuid;

use crate::common::stub_chat::{StubChat, register_stub_model};

async fn upload(
    server: &crate::common::TestServer,
    token: &str,
    filename: &str,
    content: Vec<u8>,
    mime: &str,
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content)
            .file_name(filename.to_string())
            .mime_str(mime)
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
    crate::chat::helpers::parse_uuid(&resp.json::<serde_json::Value>().await.unwrap()["id"])
}

#[tokio::test]
async fn text_attachment_is_inlined_verbatim_binary_becomes_placeholder() {
    let server = crate::common::TestServer::start().await;
    // Full perms: this user is also the admin passed to register_stub_model,
    // which additionally creates a group (POST /groups → groups::create) and
    // assigns it (groups::edit, llm_providers::assign_groups). The narrow list
    // above lacked the group perms, so POST /groups returned no id and the
    // helper panicked on unwrap. Matches the `&["*"]` stub-model convention.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "routing_user",
        &["*"],
    )
    .await;

    // Capturing stub provider (records every request's concatenated text).
    let stub = StubChat::start().await;
    let model_id = register_stub_model(
        &server,
        &user.token,
        &user.user_id,
        &stub.base_url,
        false,
        None,
    )
    .await;
    let model_id = Uuid::parse_str(&model_id).unwrap();

    // (1) A text file whose verbatim body must reach the model.
    const MARKER: &str = "VERBATIM_ROUTING_MARKER_8f3a2";
    let txt_id = upload(
        &server,
        &user.token,
        "notes.txt",
        format!("line one\n{MARKER}\nline three\n").into_bytes(),
        "text/plain",
    )
    .await;

    // (2) An unsupported binary — its raw bytes must NOT be inlined. The content
    // must be GENUINELY binary: an octet-stream whose bytes `looks_like_text`
    // (pure ASCII source/config) is legitimately text-extracted on upload and
    // inlined verbatim (the source-code-as-octet-stream fallback). A leading NUL
    // + high bytes keep `looks_like_text` false → text_page_count 0 → the routing
    // decision falls to the unsupported-binary placeholder branch.
    const SECRET_BYTES: &str = "RAW_BINARY_BYTES_SHOULD_NOT_LEAK_d91c";
    let mut bin_content = vec![0x00u8, 0xFF, 0xFE];
    bin_content.extend_from_slice(SECRET_BYTES.as_bytes());
    bin_content.extend_from_slice(&[0x00, 0xFD]);
    let bin_id = upload(
        &server,
        &user.token,
        "blob.bin",
        bin_content,
        "application/octet-stream",
    )
    .await;

    let conv = crate::chat::helpers::create_conversation(&server, &user.token, None, None).await;
    let conv_id = crate::chat::helpers::parse_uuid(&conv["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conv["active_branch_id"]);

    let events = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conv_id,
        json!({
            "model_id": model_id,
            "branch_id": branch_id,
            "content": "Analyze the attached files",
            "file_ids": [txt_id, bin_id],
        }),
        &[],
    )
    .await;
    assert_eq!(
        events.iter().filter(|e| e.event == "complete").count(),
        1,
        "attachment turn should end on one complete event"
    );

    let reqs = stub.requests();
    // The chat send also triggers a tool-less title-generation call ("Generate a
    // concise, descriptive title …") which records its OWN request and is NOT the
    // turn that carries the attachments. Select the MAIN chat request (the one
    // echoing the user's prompt, never the title generator) — asserting on
    // `reqs.last()` would inspect the title-gen request and miss the inlined file.
    let last = reqs
        .iter()
        .find(|r| {
            r.all_text.contains("Analyze the attached files")
                && !r.all_text.contains(crate::common::stub_chat::TITLE_PROMPT_PREFIX)
        })
        .expect("stub recorded the main chat request");

    // Text file: verbatim content routed inline → marker present.
    assert!(
        last.all_text.contains(MARKER),
        "text/* attachment must be inlined verbatim (Base64 text branch); all_text missing marker"
    );
    // The binary's filename is surfaced as a labeled placeholder...
    assert!(
        last.all_text.contains("blob.bin"),
        "unsupported binary should surface a labeled [File: ...] placeholder"
    );
    // ...but its raw bytes must NEVER be inlined.
    assert!(
        !last.all_text.contains(SECRET_BYTES),
        "raw bytes of an unsupported binary must NOT reach the model"
    );
}
