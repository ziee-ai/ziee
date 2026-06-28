//! Bundle-extract security through the REAL HTTP import endpoint
//! (audit all-6359198fdd5d).
//!
//! `hub/bundle.rs` has 6 in-source bomb-guard unit tests, but they call
//! the extractor (`extract_from_seed_bytes`) directly. The user-facing
//! production path is `POST /api/skills/import` (multipart `bundle`
//! field, `skills::install`) → `dev_handlers::import_skill` →
//! `bundle::extract_tarball_bytes` → `extract_tar_gz_to`, which applies
//! the SAME guards. These tests upload genuinely malicious tar.gz bodies
//! through that HTTP handler and assert the guards reject them with a 422
//! (and no skill row is created), proving the protection holds on the
//! authenticated upload path and not just in isolation.

use std::io::Cursor;

use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::multipart;
use tar::{Builder, Header};

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// A tar.gz carrying a single path-traversal entry (`../../etc/passwd`).
/// `tar::Builder` rejects `..` at `append_data` time, so — exactly like
/// the in-source `rejects_path_traversal` unit test — we inject the
/// malicious path straight into the raw GNU header to reach OUR
/// extractor's own path-safety check.
fn malicious_traversal_targz() -> Vec<u8> {
    let cur = Cursor::new(Vec::<u8>::new());
    let enc = GzEncoder::new(cur, Compression::default());
    let mut builder = Builder::new(enc);

    let mut header = Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header
        .as_gnu_mut()
        .unwrap()
        .name[..16]
        .copy_from_slice(b"../../etc/passwd");
    header.set_cksum();
    builder
        .append(&header, &b"pwned"[..])
        .expect("append raw traversal header");

    let enc = builder.into_inner().expect("into_inner");
    enc.finish().expect("gz finish").into_inner()
}

/// A tar.gz carrying a symlink entry (a non-regular entry the extractor
/// must refuse). Mirrors the in-source `rejects_symlink_entry` unit test.
fn malicious_symlink_targz() -> Vec<u8> {
    let cur = Cursor::new(Vec::<u8>::new());
    let enc = GzEncoder::new(cur, Compression::default());
    let mut builder = Builder::new(enc);

    // A legit regular file first, so the archive isn't trivially empty.
    let mut reg = Header::new_gnu();
    reg.set_size(13);
    reg.set_mode(0o644);
    reg.set_entry_type(tar::EntryType::Regular);
    builder
        .append_data(&mut reg, "SKILL.md", &b"# legit skill"[..])
        .expect("append regular");

    let mut link = Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_link_name("/etc/passwd").expect("set_link_name");
    link.set_cksum();
    builder
        .append_data(&mut link, "evil_link", std::io::empty())
        .expect("append symlink");

    let enc = builder.into_inner().expect("into_inner");
    enc.finish().expect("gz finish").into_inner()
}

async fn upload_bundle(server: &TestServer, token: &str, body: Vec<u8>) -> reqwest::Response {
    let form = multipart::Form::new().part(
        "bundle",
        multipart::Part::bytes(body)
            .file_name("bundle.tar.gz")
            .mime_str("application/gzip")
            .unwrap(),
    );
    reqwest::Client::new()
        .post(server.api_url("/skills/import"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("import request failed")
}

async fn user_skill_count(server: &TestServer, token: &str) -> usize {
    let resp = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list skills failed");
    assert_eq!(resp.status(), 200, "list skills should succeed");
    let body: serde_json::Value = resp.json().await.expect("parse skills list");
    body.get("skills")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .or_else(|| body.as_array().map(|a| a.len()))
        .unwrap_or(0)
}

/// A path-traversal bundle uploaded through `POST /api/skills/import` is
/// rejected by the extractor's path-safety guard (422), and no skill is
/// installed — the `..` entry never escapes the staging root.
#[tokio::test]
async fn import_rejects_path_traversal_bundle_over_http() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "bundle_traversal",
        &["skills::read", "skills::install"],
    )
    .await;

    let before = user_skill_count(&server, &user.token).await;
    let resp = upload_bundle(&server, &user.token, malicious_traversal_targz()).await;

    assert_eq!(
        resp.status(),
        422,
        "path-traversal bundle must be rejected through the HTTP import path"
    );
    let body: serde_json::Value = resp.json().await.expect("parse error body");
    let blob = body.to_string().to_lowercase();
    assert!(
        blob.contains("path") || blob.contains("unsafe"),
        "expected a path/unsafe rejection, got: {body}"
    );

    let after = user_skill_count(&server, &user.token).await;
    assert_eq!(after, before, "no skill row may be created by a rejected bundle");
}

/// A symlink entry uploaded through `POST /api/skills/import` is rejected
/// (non-regular entries are refused) with a 422, and no skill is installed.
#[tokio::test]
async fn import_rejects_symlink_bundle_over_http() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "bundle_symlink",
        &["skills::read", "skills::install"],
    )
    .await;

    let before = user_skill_count(&server, &user.token).await;
    let resp = upload_bundle(&server, &user.token, malicious_symlink_targz()).await;

    assert_eq!(
        resp.status(),
        422,
        "symlink bundle must be rejected through the HTTP import path"
    );
    let body: serde_json::Value = resp.json().await.expect("parse error body");
    let blob = body.to_string().to_lowercase();
    assert!(
        blob.contains("symlink") || blob.contains("non-regular") || blob.contains("not permitted"),
        "expected a symlink/non-regular rejection, got: {body}"
    );

    let after = user_skill_count(&server, &user.token).await;
    assert_eq!(after, before, "no skill row may be created by a rejected bundle");
}
