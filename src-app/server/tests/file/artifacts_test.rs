//! Integration tests for the artifacts/deliverables backend: user append-version
//! (the user side of co-editing a deliverable) and multi-format file export.

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

async fn upload_text(
    server: &crate::common::TestServer,
    token: &str,
    filename: &str,
    content: &str,
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.as_bytes().to_vec())
            .file_name(filename.to_string())
            .mime_str("text/markdown")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = resp.json().await.unwrap();
    crate::chat::helpers::parse_uuid(&body["id"])
}

async fn upload_with_mime(
    server: &crate::common::TestServer,
    token: &str,
    filename: &str,
    content: &str,
    mime: &str,
) -> Uuid {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.as_bytes().to_vec())
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
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = resp.json().await.unwrap();
    crate::chat::helpers::parse_uuid(&body["id"])
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// TEST-6b (ITEM-19, ITEM-20): appending a new version of a NON-markdown text file
// (csv / python) re-extracts its text pages so `/text` returns the edited head —
// the co-edit path for code + csv deliverables must not go stale after Save.
#[tokio::test]
async fn test_append_version_text_reextracted_for_csv_and_code() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_reextract",
        &["files::read", "files::upload", "files::download"],
    )
    .await;

    for (filename, mime, orig, edited, marker) in [
        ("data.csv", "text/csv", "name,score\nAlice,10\n", "name,score\nBob,10\n", "Bob"),
        (
            "script.py",
            "text/x-python",
            "def hello():\n    return 1\n",
            "def hello():\n    return 1\nCODE_EDIT_MARKER = 42\n",
            "CODE_EDIT_MARKER",
        ),
    ] {
        let fid = upload_with_mime(&server, &user.token, filename, orig, mime).await;
        let resp = client()
            .post(server.api_url(&format!("/files/{fid}/versions")))
            .header("Authorization", format!("Bearer {}", user.token))
            .json(&json!({ "content": edited }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{filename}: append ok");

        let text = client()
            .get(server.api_url(&format!("/files/{fid}/text")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            text.contains(marker),
            "{filename}: /text must reflect the edited head (re-extracted); got: {text}"
        );
    }
}

// TEST-10 (ITEM-1): append-version bumps the head and changes the content.
#[tokio::test]
async fn test_append_version_bumps_head_and_content() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_user",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &user.token, "report.md", "# Draft\n\nv1 content\n").await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": "# Draft\n\nEDITED content\n" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"].as_i64().unwrap(), 2, "head version bumps to 2");

    let text = client()
        .get(server.api_url(&format!("/files/{fid}/text")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(text.contains("EDITED"), "head text reflects the edit; got: {text}");
}

// TEST-10 (ITEM-1): byte-identical content is a no-op (content-addressed).
#[tokio::test]
async fn test_append_version_noop_on_identical() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_noop",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let content = "# Same\n\nunchanged\n";
    let fid = upload_text(&server, &user.token, "same.md", content).await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "content": content }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"].as_i64().unwrap(), 1, "identical content does not add a version");
}

// TEST-10 (ITEM-1): another user's file id is not found (ownership-scoped).
#[tokio::test]
async fn test_append_version_cross_user_404() {
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_owner",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_other",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &owner.token, "owned.md", "secret\n").await;

    let resp = client()
        .post(server.api_url(&format!("/files/{fid}/versions")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "content": "hijack\n" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn export_bytes(
    server: &crate::common::TestServer,
    token: &str,
    fid: Uuid,
    format: &str,
) -> (StatusCode, Vec<u8>) {
    let resp = client()
        .get(server.api_url(&format!("/files/{fid}/export?format={format}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.bytes().await.unwrap().to_vec();
    (status, bytes)
}

// TEST-12/13 (ITEM-3, ITEM-23): export a markdown deliverable to md/docx/pdf/html.
#[tokio::test]
async fn test_file_export_formats() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_export",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &user.token, "doc.md", "# Title\n\nHello **world**.\n").await;

    let (s, md) = export_bytes(&server, &user.token, fid, "md").await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8_lossy(&md).contains("# Title"), "md is the raw source");

    let (s, docx) = export_bytes(&server, &user.token, fid, "docx").await;
    assert_eq!(s, StatusCode::OK);
    assert!(docx.len() > 4 && &docx[..2] == b"PK", "docx is an OOXML zip (PK magic)");

    let (s, pdf) = export_bytes(&server, &user.token, fid, "pdf").await;
    assert_eq!(s, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"), "pdf starts with %PDF");

    let (s, html) = export_bytes(&server, &user.token, fid, "html").await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        String::from_utf8_lossy(&html).to_lowercase().contains("title"),
        "html contains the content"
    );

    let (s, _) = export_bytes(&server, &user.token, fid, "bogus").await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "unsupported format is rejected");
}

// TEST-12b (ITEM-3): an extensionless filename falls back to markdown (does not
// feed pandoc an invalid `-f <whole-filename>` and 500).
#[tokio::test]
async fn test_file_export_extensionless_defaults_markdown() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "artifacts_extless",
        &["files::read", "files::upload", "files::download"],
    )
    .await;
    let fid = upload_text(&server, &user.token, "README", "# Readme\n\nbody\n").await;
    let (s, md) = export_bytes(&server, &user.token, fid, "html").await;
    assert_eq!(s, StatusCode::OK, "extensionless file exports (md fallback), not 500");
    assert!(
        String::from_utf8_lossy(&md).to_lowercase().contains("readme"),
        "html reflects the markdown-parsed content"
    );
}

async fn create_conversation(server: &crate::common::TestServer, token: &str) -> Uuid {
    let res = client()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "title": "deliverables test" }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "create conversation: {}", res.status());
    let row: serde_json::Value = res.json().await.unwrap();
    crate::chat::helpers::parse_uuid(&row["id"])
}

async fn list_deliverables(
    server: &crate::common::TestServer,
    token: &str,
    conv: Uuid,
) -> (StatusCode, Vec<serde_json::Value>) {
    let resp = client()
        .get(server.api_url(&format!("/conversations/{conv}/deliverables")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body: Vec<serde_json::Value> = if status == StatusCode::OK {
        resp.json().await.unwrap()
    } else {
        Vec::new()
    };
    (status, body)
}

// TEST-14 (ITEM-18): pin promotes an uploaded file into the deliverables list;
// unpin removes it. Round-trips through the real REST surface.
#[tokio::test]
async fn test_deliverables_pin_list_unpin() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deliverables_user",
        &[
            "files::read",
            "files::upload",
            "files::download",
            "conversations::read",
            "conversations::edit",
        ],
    )
    .await;
    let conv = create_conversation(&server, &user.token).await;
    let fid = upload_text(&server, &user.token, "out.md", "# Deliverable\n").await;

    // Initially the (empty) conversation has no deliverables.
    let (s, list) = list_deliverables(&server, &user.token, conv).await;
    assert_eq!(s, StatusCode::OK);
    assert!(list.is_empty(), "no deliverables before pin");

    // Pin the uploaded file.
    let resp = client()
        .post(server.api_url(&format!("/conversations/{conv}/deliverables/{fid}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "pinned": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let (s, list) = list_deliverables(&server, &user.token, conv).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.len(), 1, "pinned file appears in deliverables");
    assert_eq!(
        crate::chat::helpers::parse_uuid(&list[0]["id"]),
        fid,
        "the pinned file is the one listed"
    );

    // Unpin removes it.
    let resp = client()
        .delete(server.api_url(&format!("/conversations/{conv}/deliverables/{fid}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let (_, list) = list_deliverables(&server, &user.token, conv).await;
    assert!(list.is_empty(), "unpinned file no longer listed");
}

// TEST-4 (ITEM-4): conversation export endpoint — md/docx render, format
// validation (400), permission + ownership gating (404). Exercises the
// conversation→markdown serializer + render pipeline end to end.
#[tokio::test]
async fn test_conversation_export() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_export_user",
        &["messages::read", "conversations::read", "conversations::edit"],
    )
    .await;
    let conv = create_conversation(&server, &user.token).await;

    // md export → 200 + markdown content-type.
    let resp = client()
        .get(server.api_url(&format!("/conversations/{conv}/export?format=md")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("markdown"),
        "md export is text/markdown"
    );

    // docx export → 200 + a valid OOXML zip (render pipeline via pandoc).
    let resp = client()
        .get(server.api_url(&format!("/conversations/{conv}/export?format=docx")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let docx = resp.bytes().await.unwrap();
    assert!(docx.len() > 4 && &docx[..2] == b"PK", "docx is a PK zip");

    // Unsupported format → 400.
    let resp = client()
        .get(server.api_url(&format!("/conversations/{conv}/export?format=bogus")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Another user cannot export this conversation → 404 (owner-scoped).
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "conv_export_other",
        &["messages::read", "conversations::read"],
    )
    .await;
    let resp = client()
        .get(server.api_url(&format!("/conversations/{conv}/export?format=md")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// TEST-14 (ITEM-18, ownership): another user cannot read or pin into a
// conversation they don't own (owner-scoped → 404).
#[tokio::test]
async fn test_deliverables_cross_user_scoped() {
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deliverables_owner",
        &["files::read", "files::upload", "conversations::read", "conversations::edit"],
    )
    .await;
    let other = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deliverables_other",
        &["files::read", "files::upload", "conversations::read", "conversations::edit"],
    )
    .await;
    let conv = create_conversation(&server, &owner.token).await;
    let fid = upload_text(&server, &owner.token, "owned.md", "secret\n").await;

    // Other user reading the owner's conversation deliverables → 404 (not leaked).
    let (s, _) = list_deliverables(&server, &other.token, conv).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "cross-user list is 404");

    // Other user pinning into the owner's conversation → 404.
    let resp = client()
        .post(server.api_url(&format!("/conversations/{conv}/deliverables/{fid}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&json!({ "pinned": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "cross-user pin is 404");
}
