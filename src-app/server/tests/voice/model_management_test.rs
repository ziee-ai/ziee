//! Integration tests for the whisper **MODEL management** admin surface
//! (`/api/voice/models/*` + a few version/instance reads), mapping to TESTS.md
//! TEST-6..13, TEST-25/26/29/30/32/34/35.
//!
//! Philosophy (mirrors `version_download_test` / `settings_test`): drive the FULL
//! REST → download-task → verify → register path for real, mocking ONLY the
//! external boundary — a loopback HTTP server that serves BOTH the HuggingFace
//! **tree API** (via `WHISPER_CATALOG_MIRROR`) and the model **file** bytes (via
//! `WHISPER_MODEL_MIRROR`). Fixture bytes carry the real `ggml` magic and their
//! real sha256 is advertised as the tree `oid`, so oid-verification passes for
//! real. No network, no paid credentials.
//!
//! SSRF note: the user-supplied **arbitrary-URL** branch (`{url}`) is validated
//! against `PUBLIC_HTTP_OR_HTTPS`, which *rejects loopback* — so it can't reach
//! our loopback mock (that's exactly what TEST-9 asserts). The "unverified
//! storage (verified=false + computed sha256)" behaviour that a URL download
//! produces is therefore exercised through the loopback-reachable **catalog
//! entry with no advertised oid** (same `download_model_file(expected_sha256:
//! None)` code path; the task even records it with `source='url'`). This is
//! called out at each such test.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as AxPath, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{insert_version_row, VOICE_ADMIN_PERMS};
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

// ───────────────────────── loopback model mirror ─────────────────────────

/// One file the mock serves + advertises in the tree.
#[derive(Clone)]
struct FileFixture {
    /// Short model name (`req.name` / catalog `name`).
    name: String,
    /// On-disk + served filename (`ggml-<name>.bin`).
    filename: String,
    bytes: Vec<u8>,
    /// Advertise `lfs.oid = sha256(bytes)` in the tree (→ catalog download is
    /// oid-verified, stored `verified=true`, `source='catalog'`). When false the
    /// tree lists a plain (non-LFS) file → download stores `verified=false`,
    /// `source='url'` (the unverified path a raw-URL download also takes).
    advertise_oid: bool,
    /// Stream the body slowly (chunked, with per-chunk sleeps) so a download
    /// stays observably active long enough to list / cancel it.
    slow: bool,
}

#[derive(Clone)]
struct MockState {
    files: Arc<Vec<FileFixture>>,
}

/// A running loopback mock + its base URL. Aborts the server task on drop.
struct MockMirror {
    base_url: String,
    files: Vec<FileFixture>,
    _handle: JoinHandle<()>,
}

impl Drop for MockMirror {
    fn drop(&mut self) {
        self._handle.abort();
    }
}

impl MockMirror {
    fn sha256_of(&self, name: &str) -> String {
        let f = self.files.iter().find(|f| f.name == name).unwrap();
        hex_sha256(&f.bytes)
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Default fixture set covering every model-download scenario in one mock.
fn default_fixtures() -> Vec<FileFixture> {
    // A valid ggml body: the 4-byte magic + deterministic filler (so its sha256
    // is stable across runs and we can advertise it as the tree oid).
    let ggml = |tag: &str, fill: usize| {
        let mut v = Vec::with_capacity(4 + fill);
        v.extend_from_slice(b"ggml");
        let seed = tag.as_bytes();
        for i in 0..fill {
            v.push(seed[i % seed.len()] ^ (i as u8));
        }
        v
    };
    vec![
        // Verified catalog model (has oid) → verified=true, source=catalog.
        FileFixture {
            name: "verok".to_string(),
            filename: "ggml-verok.bin".to_string(),
            bytes: ggml("verok-model", 4096),
            advertise_oid: true,
            slow: false,
        },
        // Catalog entry WITHOUT an advertised oid → verified=false, source=url
        // (the unverified-storage path). Named to make the intent obvious.
        FileFixture {
            name: "unverif".to_string(),
            filename: "ggml-unverif.bin".to_string(),
            bytes: ggml("unverified-model", 2048),
            advertise_oid: false,
            slow: false,
        },
        // A ggml-*.bin whose BYTES are not a whisper model (bad magic) → the
        // download's magic check rejects it, no row is created.
        FileFixture {
            name: "badmagic".to_string(),
            filename: "ggml-badmagic.bin".to_string(),
            bytes: b"<!DOCTYPE html> this is not a whisper model at all".to_vec(),
            advertise_oid: false,
            slow: false,
        },
        // A larger body streamed slowly (has oid → verified=true) so a download
        // is observably active for the list / cancel tests.
        FileFixture {
            name: "slow".to_string(),
            filename: "ggml-slow.bin".to_string(),
            bytes: ggml("slow-streaming-model", 256 * 1024),
            advertise_oid: true,
            slow: true,
        },
    ]
}

/// Build the HF tree-API JSON body advertised for `files`.
fn tree_json(files: &[FileFixture]) -> String {
    let entries: Vec<Value> = files
        .iter()
        .map(|f| {
            if f.advertise_oid {
                json!({
                    "type": "file",
                    "path": f.filename,
                    "size": f.bytes.len(),
                    "lfs": { "oid": hex_sha256(&f.bytes), "size": f.bytes.len() }
                })
            } else {
                json!({ "type": "file", "path": f.filename, "size": f.bytes.len() })
            }
        })
        .collect();
    serde_json::to_string(&entries).unwrap()
}

async fn mock_route(
    State(state): State<MockState>,
    AxPath(path): AxPath<String>,
) -> axum::response::Response {
    // Tree-API request: `{owner}/{repo}/tree/main`.
    if path.ends_with("/tree/main") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            tree_json(&state.files),
        )
            .into_response();
    }
    // File request: the last path segment is the filename.
    let filename = path.rsplit('/').next().unwrap_or("").to_string();
    let Some(f) = state.files.iter().find(|f| f.filename == filename).cloned() else {
        return (StatusCode::NOT_FOUND, format!("no fixture for {filename}")).into_response();
    };
    if f.slow {
        // Stream in ~10 KiB chunks with a per-chunk sleep so the whole body takes
        // ~1.5s — long enough to observe an active download + race a cancel.
        let bytes = f.bytes.clone();
        let stream = async_stream::stream! {
            for chunk in bytes.chunks(10 * 1024) {
                tokio::time::sleep(Duration::from_millis(80)).await;
                yield Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(chunk));
            }
        };
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            axum::body::Body::from_stream(stream),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/octet-stream")],
        f.bytes,
    )
        .into_response()
}

/// Stand up the loopback mock serving the default fixtures. The tree JSON is
/// returned for ANY `.../tree/main` path, so it works under whatever source repo
/// the server requests (default `ggerganov/whisper.cpp`).
async fn spawn_mock() -> MockMirror {
    let files = default_fixtures();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let port = listener.local_addr().unwrap().port();
    let app = axum::Router::new()
        .route("/{*path}", get(mock_route))
        .with_state(MockState {
            files: Arc::new(files.clone()),
        });
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    MockMirror {
        base_url: format!("http://127.0.0.1:{port}"),
        files,
        _handle: handle,
    }
}

/// Start a TestServer wired to the mock (both mirror env vars point at it), plus
/// any extra env.
async fn server_with_mirror(mock: &MockMirror, extra: Vec<(String, String)>) -> TestServer {
    let mut env = vec![
        ("WHISPER_CATALOG_MIRROR".to_string(), mock.base_url.clone()),
        ("WHISPER_MODEL_MIRROR".to_string(), mock.base_url.clone()),
    ];
    env.extend(extra);
    TestServer::start_with_options(TestServerOptions {
        extra_env: env,
        ..Default::default()
    })
    .await
}

// ───────────────────────────── request helpers ───────────────────────────

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn post_download(server: &TestServer, token: &str, body: Value) -> reqwest::Response {
    client()
        .post(server.api_url("/voice/models/download"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("download request")
}

async fn get_json(server: &TestServer, token: &str, path: &str) -> (StatusCode, Value) {
    let r = client()
        .get(server.api_url(path))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get");
    let st = r.status();
    let v = r.json::<Value>().await.unwrap_or(Value::Null);
    (
        StatusCode::from_u16(st.as_u16()).unwrap(),
        v,
    )
}

/// Multipart-upload a model file (optionally omitting the name/file field).
async fn upload_model(
    server: &TestServer,
    token: &str,
    name: Option<&str>,
    file: Option<(&str, Vec<u8>)>,
) -> reqwest::Response {
    let mut form = reqwest::multipart::Form::new();
    if let Some(n) = name {
        form = form.text("name", n.to_string());
    }
    if let Some((fname, bytes)) = file {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(fname.to_string())
            .mime_str("application/octet-stream")
            .unwrap();
        form = form.part("file", part);
    }
    client()
        .post(server.api_url("/voice/models/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("upload request")
}

/// Drive the MODEL download-events SSE to a terminal frame. `Ok(())` on
/// `complete`, `Err(msg)` on `failed`. Mirrors `drive_download_to_terminal`
/// (which targets the *version* SSE), but for `/voice/models/downloads/{key}/events`.
async fn drive_model_download(
    server: &TestServer,
    token: &str,
    key: &str,
    timeout: Duration,
) -> Result<(), String> {
    use tokio_stream::StreamExt;
    let url = server.api_url(&format!("/voice/models/downloads/{key}/events"));
    let resp = client()
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("subscribe model download events");
    assert_eq!(resp.status(), 200, "model download-events SSE should 200");

    let deadline = tokio::time::Instant::now() + timeout;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for a terminal model-download SSE frame");
        }
        let chunk = match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => panic!("SSE stream error: {e}"),
            Ok(None) => panic!("SSE closed before a terminal frame"),
            Err(_) => panic!("timed out reading the model-download SSE stream"),
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find("\n\n") {
            let frame: String = buf.drain(..pos + 2).collect();
            let event = frame
                .lines()
                .find_map(|l| l.strip_prefix("event:").map(|r| r.trim().to_string()));
            match event.as_deref() {
                Some("complete") => return Ok(()),
                Some("failed") => {
                    let data = frame
                        .lines()
                        .find_map(|l| l.strip_prefix("data:").map(|r| r.trim().to_string()))
                        .unwrap_or_default();
                    return Err(data);
                }
                _ => {}
            }
        }
    }
}

/// GET /voice/models as the given token, returning the array.
async fn list_models(server: &TestServer, token: &str) -> Vec<Value> {
    let (st, v) = get_json(server, token, "/voice/models").await;
    assert_eq!(st, StatusCode::OK, "GET /voice/models should 200: {v:?}");
    v.as_array().cloned().unwrap_or_default()
}

/// The on-disk voice-models dir for a spawned server.
fn voice_models_dir(server: &TestServer) -> std::path::PathBuf {
    server.data_dir().join("voice-models")
}

// ═══════════════════════════════ TEST-6 ══════════════════════════════════

/// TEST-6 — download a catalog model (oid-verified) via the mock, then
/// `GET /models` lists it (`source='catalog'`, `verified=true`); re-downloading
/// the same filename upserts in place (no double-insert).
#[tokio::test]
async fn test_6_catalog_download_lists_and_dedups() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t6", VOICE_ADMIN_PERMS).await;

    // Download the verified catalog model.
    let res = post_download(&server, &admin.token, json!({ "name": "verok" })).await;
    assert_eq!(res.status(), 200, "catalog download start should 200");
    let started: Value = res.json().await.unwrap();
    let key = started["key"].as_str().unwrap().to_string();
    assert_eq!(key, "ggml-verok.bin", "key == target filename");
    drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("catalog download should complete");

    // Listed once, catalog + verified, with the advertised size + recorded sha.
    let models = list_models(&server, &admin.token).await;
    let verok: Vec<_> = models.iter().filter(|m| m["name"] == "verok").collect();
    assert_eq!(verok.len(), 1, "exactly one row for the downloaded model");
    let row = verok[0];
    assert_eq!(row["source"], "catalog");
    assert_eq!(row["verified"], true, "oid-verified download is verified=true");
    assert_eq!(row["filename"], "ggml-verok.bin");
    assert_eq!(
        row["sha256"].as_str().unwrap(),
        mock.sha256_of("verok"),
        "recorded sha == the fixture's real digest"
    );
    // The file is on disk.
    assert!(voice_models_dir(&server).join("ggml-verok.bin").exists());

    // Re-download the same model → upsert on filename, still ONE row.
    let res = post_download(&server, &admin.token, json!({ "name": "verok" })).await;
    assert_eq!(res.status(), 200);
    let key2 = res.json::<Value>().await.unwrap()["key"].as_str().unwrap().to_string();
    drive_model_download(&server, &admin.token, &key2, Duration::from_secs(30))
        .await
        .expect("re-download completes");
    let models = list_models(&server, &admin.token).await;
    assert_eq!(
        models.iter().filter(|m| m["name"] == "verok").count(),
        1,
        "re-download does not double-insert"
    );
}

// ═══════════════════════════════ TEST-7 ══════════════════════════════════

/// TEST-7 — `POST /models/download` returns `{task_id,key,events_url}`;
/// `GET /models/downloads` lists it active; the `/events` SSE yields
/// connected→progress→complete; a `voice_models` row is registered.
#[tokio::test]
async fn test_7_download_start_active_list_sse_and_row() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t7", VOICE_ADMIN_PERMS).await;

    // Slow catalog model so the download is reliably observable as "active".
    let res = post_download(&server, &admin.token, json!({ "name": "slow" })).await;
    assert_eq!(res.status(), 200);
    let started: Value = res.json().await.unwrap();
    assert!(started["task_id"].as_str().is_some(), "task_id present");
    let key = started["key"].as_str().unwrap().to_string();
    assert_eq!(
        started["events_url"],
        format!("/api/voice/models/downloads/{key}/events"),
        "events_url points at the SSE endpoint"
    );

    // While it streams (bounded poll), the active-downloads list carries the key.
    let mut saw_active = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let (st, v) = get_json(&server, &admin.token, "/voice/models/downloads").await;
        assert_eq!(st, StatusCode::OK);
        if v.as_array().map(|a| a.iter().any(|d| d["key"] == key)).unwrap_or(false) {
            saw_active = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(saw_active, "the in-flight download appears in GET /models/downloads");

    // The SSE drives to complete (connected→progress→complete). The helper
    // already asserts a 200 handshake + waits for the terminal `complete`.
    drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("slow download should complete");

    // A row is now registered for the model.
    let models = list_models(&server, &admin.token).await;
    assert!(
        models.iter().any(|m| m["name"] == "slow"),
        "voice_models row registered on completion"
    );
    // And the completed snapshot is still queryable (see also TEST-34).
    let (st, snap) = get_json(&server, &admin.token, &format!("/voice/models/downloads/{key}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(snap["status"], "completed", "terminal snapshot is completed");
}

// ═══════════════════════════════ TEST-8 ══════════════════════════════════

/// TEST-8 — an UNVERIFIED download (catalog entry with no advertised oid; the
/// same `expected_sha256:None` path a raw-URL download takes, and recorded with
/// `source='url'`) stores `verified=false` + the computed sha256. A ggml-named
/// entry whose bytes fail the magic check is rejected with NO row created.
///
/// (The literal raw-`{url}` branch is SSRF-restricted to public hosts and so
/// can't reach the loopback mock — that restriction is TEST-9. This exercises
/// the identical unverified-storage code path via a loopback-reachable source.)
#[tokio::test]
async fn test_8_unverified_stores_computed_sha_and_bad_magic_rejected() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t8", VOICE_ADMIN_PERMS).await;

    // Unverified (no oid) download → verified=false + computed sha, source=url.
    let res = post_download(&server, &admin.token, json!({ "name": "unverif" })).await;
    assert_eq!(res.status(), 200);
    let key = res.json::<Value>().await.unwrap()["key"].as_str().unwrap().to_string();
    drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("unverified download should still complete");

    let models = list_models(&server, &admin.token).await;
    let row = models.iter().find(|m| m["name"] == "unverif").expect("row present");
    assert_eq!(row["verified"], false, "no oid → verified=false");
    assert_eq!(row["source"], "url", "unverified remote fetch recorded as source=url");
    assert_eq!(
        row["sha256"].as_str().unwrap(),
        mock.sha256_of("unverif"),
        "the computed sha256 of the bytes is stored"
    );

    // Bad-magic body → rejected via SSE `failed`, NO row created.
    let res = post_download(&server, &admin.token, json!({ "name": "badmagic" })).await;
    assert_eq!(res.status(), 200, "download starts (magic checked in-stream)");
    let key = res.json::<Value>().await.unwrap()["key"].as_str().unwrap().to_string();
    let err = drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect_err("bad-magic body must fail");
    assert!(
        err.to_lowercase().contains("magic") || err.to_lowercase().contains("whisper"),
        "failure names the bad magic, got: {err}"
    );
    let models = list_models(&server, &admin.token).await;
    assert!(
        !models.iter().any(|m| m["name"] == "badmagic"),
        "a rejected download creates no row"
    );
    assert!(
        !voice_models_dir(&server).join("ggml-badmagic.bin").exists(),
        "no file left behind for the rejected download"
    );
}

// ═══════════════════════════════ TEST-9 ══════════════════════════════════

/// TEST-9 — a user-supplied arbitrary-URL download targeting IMDS / loopback is
/// refused by the SSRF policy (`PUBLIC_HTTP_OR_HTTPS`): the fetch never happens
/// and the SSE terminates in `failed` with a clear reason; no row is created.
#[tokio::test]
async fn test_9_arbitrary_url_ssrf_refused() {
    let server = TestServer::start().await; // no mirror needed; the URL is verbatim
    let admin = create_user_with_permissions(&server, "voice_mm_t9", VOICE_ADMIN_PERMS).await;

    for bad_url in [
        "http://169.254.169.254/latest/meta-data/ggml-x.bin", // IMDS
        "http://127.0.0.1:1/ggml-x.bin",                       // loopback
    ] {
        let res = post_download(
            &server,
            &admin.token,
            json!({ "name": "evil", "url": bad_url }),
        )
        .await;
        assert_eq!(res.status(), 200, "start returns 200 (SSRF checked in-task)");
        let key = res.json::<Value>().await.unwrap()["key"].as_str().unwrap().to_string();
        let err = drive_model_download(&server, &admin.token, &key, Duration::from_secs(15))
            .await
            .expect_err("SSRF target must fail");
        assert!(
            err.to_uppercase().contains("SSRF") || err.to_lowercase().contains("rejected"),
            "failure is the SSRF rejection, got: {err}"
        );
    }

    let models = list_models(&server, &admin.token).await;
    assert!(models.is_empty(), "no rows created for SSRF-refused downloads");
}

// ═══════════════════════════════ TEST-10 ═════════════════════════════════

/// TEST-10 — `POST /models/upload` multipart with a valid ggml header stores the
/// file + inserts a row (`source='upload'`, `verified=false`); a bad-magic body
/// returns 4xx and creates nothing.
///
/// The over-cap arm is the 5 GiB `VOICE_MODEL_MAX_UPLOAD_BYTES` ceiling —
/// infeasible to stream in an integration test; the constant + its enforcement
/// are unit-covered in `model.rs` (`upload_cap_is_a_sane_bound`). We exercise the
/// reachable 4xx arm (bad magic) here.
#[tokio::test]
async fn test_10_upload_valid_and_bad_magic() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t10", VOICE_ADMIN_PERMS).await;

    // Valid ggml upload.
    let mut good = b"ggml".to_vec();
    good.extend_from_slice(b"uploaded-whisper-model-bytes-0123456789");
    let res = upload_model(&server, &admin.token, Some("myupload"), Some(("model.bin", good.clone()))).await;
    assert_eq!(res.status(), 200, "valid upload should 200");
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["name"], "myupload");
    assert_eq!(row["source"], "upload");
    assert_eq!(row["verified"], false, "uploads are unverified");
    assert_eq!(row["filename"], "ggml-myupload.bin");
    assert!(
        voice_models_dir(&server).join("ggml-myupload.bin").exists(),
        "uploaded file is on disk"
    );
    assert!(
        list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "myupload"),
        "upload is listed"
    );

    // Bad-magic upload → 4xx, no row.
    let res = upload_model(
        &server,
        &admin.token,
        Some("junkupload"),
        Some(("junk.bin", b"NOT-A-GGML-MODEL-FILE".to_vec())),
    )
    .await;
    assert!(res.status().is_client_error(), "bad magic → 4xx, got {}", res.status());
    assert!(
        !list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "junkupload"),
        "a rejected upload creates no row"
    );
    assert!(!voice_models_dir(&server).join("ggml-junkupload.bin").exists());

    // Missing file field → 4xx.
    let res = upload_model(&server, &admin.token, Some("nofile"), None).await;
    assert!(res.status().is_client_error(), "missing file → 4xx");

    // No leaked temp files remain.
    assert_no_temp_leak(&server);
}

// ═══════════════════════════════ TEST-11 ═════════════════════════════════

/// TEST-11 — `GET /models`, `POST /models/{id}/activate` sets
/// `voice_runtime_settings.model` to the row name; `DELETE /models/{id}` removes
/// row + file; deleting the ACTIVE model without `ack_active=true` is refused (409).
#[tokio::test]
async fn test_11_activate_sets_settings_and_delete_guard() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t11", VOICE_ADMIN_PERMS).await;

    // Two uploaded models to switch between.
    let mut a = b"ggml".to_vec();
    a.extend_from_slice(b"model-A-bytes");
    let mut b = b"ggml".to_vec();
    b.extend_from_slice(b"model-B-bytes");
    let id_a = upload_model(&server, &admin.token, Some("mdla"), Some(("a.bin", a)))
        .await
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id_b = upload_model(&server, &admin.token, Some("mdlb"), Some(("b.bin", b)))
        .await
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Activate A → settings.model == "mdla".
    let res = client()
        .post(server.api_url(&format!("/voice/models/{id_a}/activate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "activate should 200");
    let (_, settings) = get_json(&server, &admin.token, "/voice/settings").await;
    assert_eq!(settings["model"], "mdla", "activate updates settings.model");
    // The activated row reads back as active.
    let models = list_models(&server, &admin.token).await;
    assert_eq!(
        models.iter().find(|m| m["name"] == "mdla").unwrap()["is_active"],
        true
    );

    // Deleting the ACTIVE model without ack → 409.
    let res = client()
        .delete(server.api_url(&format!("/voice/models/{id_a}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409, "delete active model without ack_active → 409");

    // Deleting a NON-active model is clean (204) + removes the file.
    assert!(voice_models_dir(&server).join("ggml-mdlb.bin").exists());
    let res = client()
        .delete(server.api_url(&format!("/voice/models/{id_b}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "delete non-active model → 204");
    assert!(
        !voice_models_dir(&server).join("ggml-mdlb.bin").exists(),
        "the file is removed on delete"
    );
    assert!(!list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "mdlb"));

    // Deleting the active model WITH ack → 204 + file gone.
    let res = client()
        .delete(server.api_url(&format!("/voice/models/{id_a}?ack_active=true")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "delete active with ack_active=true → 204");
    assert!(!voice_models_dir(&server).join("ggml-mdla.bin").exists());
    assert!(!list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "mdla"));
}

// ═══════════════════════════════ TEST-12 ═════════════════════════════════

/// TEST-12 (A9 deny) — a user WITHOUT `voice::admin::manage` gets 403 on
/// download/upload/delete/activate; a user WITHOUT `voice::admin::read` gets 403
/// on list/downloads. A read-only admin is allowed the reads (positive control).
#[tokio::test]
async fn test_12_permission_denials() {
    let server = TestServer::start().await;
    // read-only admin: has read, NOT manage.
    let reader = create_user_with_permissions(&server, "voice_mm_reader", &["voice::admin::read"]).await;
    // plain Users member: neither admin perm (holds only voice::transcribe).
    let plain = create_user_with_permissions(&server, "voice_mm_plain", &[]).await;
    let rand = Uuid::new_v4();

    // ── manage-gated writes: 403 for the read-only admin ──
    let r = post_download(&server, &reader.token, json!({ "name": "verok" })).await;
    assert_eq!(r.status(), 403, "download needs manage");
    let r = upload_model(&server, &reader.token, Some("x"), Some(("x.bin", b"ggml....".to_vec()))).await;
    assert_eq!(r.status(), 403, "upload needs manage");
    let r = client()
        .post(server.api_url(&format!("/voice/models/{rand}/activate")))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "activate needs manage");
    let r = client()
        .delete(server.api_url(&format!("/voice/models/{rand}")))
        .header("Authorization", format!("Bearer {}", reader.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "delete needs manage");

    // ── read-gated reads: 403 for the plain member ──
    let (st, _) = get_json(&server, &plain.token, "/voice/models").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "list needs read");
    let (st, _) = get_json(&server, &plain.token, "/voice/models/downloads").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "downloads list needs read");
    let (st, _) = get_json(&server, &plain.token, "/voice/models/catalog").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "catalog needs read");

    // Positive control: the read-only admin CAN read.
    let (st, _) = get_json(&server, &reader.token, "/voice/models").await;
    assert_eq!(st, StatusCode::OK, "read-only admin can list models");
}

// ═══════════════════════════════ TEST-13 ═════════════════════════════════

/// TEST-13 (+ TEST-36 sync_emit) — `sync:voice_model` Create is emitted on
/// download-complete AND on upload; `sync:voice_settings` (+ `voice_model` update)
/// is emitted on activate, delivered to the admin `SyncProbe`. This IS the
/// sync-emit audience coverage (ITEM-10) enumerated as TEST-36.
#[tokio::test]
async fn test_13_sync_emits() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t13", VOICE_ADMIN_PERMS).await;

    // Subscribe BEFORE the mutations so no emit is missed.
    let mut probe = SyncProbe::open(&server, &admin.token).await;

    // (a) download-complete → voice_model/create.
    let key = post_download(&server, &admin.token, json!({ "name": "verok" }))
        .await
        .json::<Value>()
        .await
        .unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();
    drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("download completes");
    probe
        .expect_event("voice_model", "create", Duration::from_secs(5))
        .await;

    // (b) upload → voice_model/create.
    let mut good = b"ggml".to_vec();
    good.extend_from_slice(b"sync-upload-bytes");
    let id = upload_model(&server, &admin.token, Some("syncup"), Some(("u.bin", good)))
        .await
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    probe
        .expect_event("voice_model", "create", Duration::from_secs(5))
        .await;

    // (c) activate → voice_settings/update (and a voice_model/update).
    let res = client()
        .post(server.api_url(&format!("/voice/models/{id}/activate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    probe
        .expect_event("voice_settings", "update", Duration::from_secs(5))
        .await;
}

// ═══════════════════════════════ TEST-25 ═════════════════════════════════

/// TEST-25 — `PUT /voice/settings` accepts a valid `model_source_repo`
/// (persists) and rejects a malformed one (400).
#[tokio::test]
async fn test_25_model_source_repo_validation() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t25", VOICE_ADMIN_PERMS).await;

    // Default is the upstream slug.
    let (_, s) = get_json(&server, &admin.token, "/voice/settings").await;
    assert_eq!(s["model_source_repo"], "ggerganov/whisper.cpp");

    // Valid slug + valid https URL both persist.
    for good in ["myorg/mymirror", "https://hf.internal/mirror"] {
        let res = client()
            .put(server.api_url("/voice/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({ "model_source_repo": good }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200, "valid model_source_repo {good} accepted");
        let (_, s) = get_json(&server, &admin.token, "/voice/settings").await;
        assert_eq!(s["model_source_repo"], good, "value persisted");
    }

    // Malformed values → 400.
    for bad in ["no-slash", "a/b/c", "http://insecure/x", "../evil", ""] {
        let res = client()
            .put(server.api_url("/voice/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({ "model_source_repo": bad }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "malformed model_source_repo {bad:?} → 400");
    }
}

// ═══════════════════════════════ TEST-26 ═════════════════════════════════

/// TEST-26 — when the configured source is unreachable, `GET /models/catalog`
/// returns `source_reachable:false` + empty models (no 5xx); upload + the
/// installed list still work.
#[tokio::test]
async fn test_26_catalog_graceful_degrade_when_unreachable() {
    // Point the catalog mirror at a dead loopback port (nothing listens on :1).
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WHISPER_CATALOG_MIRROR".to_string(), "http://127.0.0.1:1".to_string())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "voice_mm_t26", VOICE_ADMIN_PERMS).await;

    let (st, v) = get_json(&server, &admin.token, "/voice/models/catalog").await;
    assert_eq!(st, StatusCode::OK, "catalog degrades gracefully, never 5xx");
    assert_eq!(v["source_reachable"], false, "unreachable source flagged");
    assert_eq!(
        v["models"].as_array().map(|a| a.len()).unwrap_or(999),
        0,
        "no models when unreachable"
    );

    // Upload still works with an unreachable catalog.
    let mut good = b"ggml".to_vec();
    good.extend_from_slice(b"offline-upload");
    let res = upload_model(&server, &admin.token, Some("offline"), Some(("o.bin", good))).await;
    assert_eq!(res.status(), 200, "upload works regardless of catalog reachability");
    // Installed list still works.
    assert!(
        list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "offline"),
        "installed list works with an unreachable catalog"
    );
}

// ═══════════════════════════════ TEST-29 ═════════════════════════════════

/// TEST-29 — `POST /models/downloads/{key}/cancel` aborts an in-flight download
/// (202) and leaves no `.tmp`/`.upload` files behind; cancelling an unknown key
/// → 404.
#[tokio::test]
async fn test_29_cancel_download() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t29", VOICE_ADMIN_PERMS).await;

    // Unknown key → 404.
    let res = client()
        .post(server.api_url("/voice/models/downloads/nope-nonexistent.bin/cancel"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "cancel of an unknown key → 404");

    // Start the SLOW catalog download.
    let key = post_download(&server, &admin.token, json!({ "name": "slow" }))
        .await
        .json::<Value>()
        .await
        .unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();

    // Wait until it's active, then cancel → 202.
    let mut cancelled_202 = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        let (_, v) = get_json(&server, &admin.token, "/voice/models/downloads").await;
        if v.as_array().map(|a| a.iter().any(|d| d["key"] == key)).unwrap_or(false) {
            let res = client()
                .post(server.api_url(&format!("/voice/models/downloads/{key}/cancel")))
                .header("Authorization", format!("Bearer {}", admin.token))
                .send()
                .await
                .unwrap();
            assert_eq!(res.status(), 202, "cancel of an active key → 202");
            cancelled_202 = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert!(cancelled_202, "the slow download was observed active + cancelled");

    // The download terminates in `failed` (cancelled); no row registered.
    let err = drive_model_download(&server, &admin.token, &key, Duration::from_secs(15))
        .await
        .expect_err("a cancelled download ends in failed");
    assert!(
        err.to_lowercase().contains("cancel"),
        "failure reason mentions cancellation, got: {err}"
    );
    assert!(
        !list_models(&server, &admin.token).await.iter().any(|m| m["name"] == "slow"),
        "a cancelled download registers no model row"
    );
    // No temp/partial files leaked in the voice-models dir.
    assert_no_temp_leak(&server);
}

/// Assert the voice-models dir holds no `*.tmp` / `.upload-*` partials.
fn assert_no_temp_leak(server: &TestServer) {
    let dir = voice_models_dir(server);
    if !dir.exists() {
        return;
    }
    for entry in std::fs::read_dir(&dir).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        assert!(
            !name.ends_with(".tmp") && !name.starts_with(".upload"),
            "leaked temp/partial file in voice-models: {name}"
        );
    }
}

// ═══════════════════════════════ TEST-32 ═════════════════════════════════

/// TEST-32 — migration 155's CHECK on `voice_runtime_instance.state` rejects an
/// out-of-vocab value (a direct sqlx UPDATE to an invalid state errors); valid
/// state names are accepted.
#[tokio::test]
async fn test_32_instance_state_check_constraint() {
    let server = TestServer::start().await;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("connect test db");

    // An out-of-vocabulary state violates the CHECK.
    let bad = sqlx::query("UPDATE voice_runtime_instance SET state = 'bogus-state' WHERE id = TRUE")
        .execute(&pool)
        .await;
    assert!(bad.is_err(), "an out-of-vocab state must violate the CHECK");

    // Every allowed value is accepted.
    for st in [
        "starting", "healthy", "unhealthy", "crashed", "restarting", "failed", "stopped",
    ] {
        let ok = sqlx::query("UPDATE voice_runtime_instance SET state = $1 WHERE id = TRUE")
            .bind(st)
            .execute(&pool)
            .await;
        assert!(ok.is_ok(), "valid state {st} must be accepted: {ok:?}");
    }
    pool.close().await;
}

// ═══════════════════════════════ TEST-34 ═════════════════════════════════

/// TEST-34 — `GET /models/downloads/{key}` returns a snapshot (200 for a known
/// key, 404 for unknown); the version snapshot endpoint
/// `GET /versions/downloads/{key}` behaves the same (404 for unknown).
#[tokio::test]
async fn test_34_download_snapshots() {
    let mock = spawn_mock().await;
    let server = server_with_mirror(&mock, vec![]).await;
    let admin = create_user_with_permissions(&server, "voice_mm_t34", VOICE_ADMIN_PERMS).await;

    // Drive a model download to completion, then snapshot its key.
    let key = post_download(&server, &admin.token, json!({ "name": "verok" }))
        .await
        .json::<Value>()
        .await
        .unwrap()["key"]
        .as_str()
        .unwrap()
        .to_string();
    drive_model_download(&server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("download completes");

    let (st, snap) = get_json(&server, &admin.token, &format!("/voice/models/downloads/{key}")).await;
    assert_eq!(st, StatusCode::OK, "model snapshot for a known key → 200");
    assert_eq!(snap["key"], key);
    assert_eq!(snap["status"], "completed");
    assert!(snap["task_id"].as_str().is_some());

    // Unknown model key → 404.
    let (st, _) = get_json(&server, &admin.token, "/voice/models/downloads/unknown-key.bin").await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown model download key → 404");

    // Version snapshot endpoint: unknown key → 404 (the happy version-download
    // snapshot is exercised by version_download_test's pipeline; here we assert
    // the snapshot route's not-found behaviour + auth).
    let (st, _) = get_json(&server, &admin.token, "/voice/versions/downloads/whisper@v0@cpu").await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown version download key → 404");
}

// ═══════════════════════════════ TEST-35 ═════════════════════════════════

/// TEST-35 — `GET /versions/{id}` returns a version (404 for an unknown id);
/// `GET /voice/instance` includes the `pid` / `uptime_seconds` fields.
#[tokio::test]
async fn test_35_get_version_and_instance_fields() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t35", VOICE_ADMIN_PERMS).await;

    // Insert a version row, fetch it by id.
    let id = insert_version_row(&server, "v9.9.9-t35", "cpu", "/some/whisper-server", false).await;
    let (st, v) = get_json(&server, &admin.token, &format!("/voice/versions/{id}")).await;
    assert_eq!(st, StatusCode::OK, "GET /versions/{{id}} → 200");
    assert_eq!(v["version"], "v9.9.9-t35");
    assert_eq!(v["backend"], "cpu");

    // Unknown id → 404.
    let (st, _) = get_json(&server, &admin.token, &format!("/voice/versions/{}", Uuid::new_v4())).await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown version id → 404");

    // The instance snapshot carries the F10 pid + uptime fields (null when stopped).
    let (st, info) = get_json(&server, &admin.token, "/voice/instance").await;
    assert_eq!(st, StatusCode::OK);
    assert!(info.get("pid").is_some(), "instance snapshot has a pid field");
    assert!(
        info.get("uptime_seconds").is_some(),
        "instance snapshot has an uptime_seconds field"
    );
    assert_eq!(info["status"], "stopped", "no instance running → stopped");
}

// ═══════════════════════════════ TEST-30 ═════════════════════════════════

/// TEST-30 (drain/activate) — activating a different model while a whisper-server
/// is running DRAINS + stops it (the next transcribe respawns against the
/// newly-activated model — the real lazy flow: `apply_active_model_change`
/// drains-and-stops, it does not eagerly respawn). Uses the REAL auto-start path
/// with the freshly-built `stub-whisper-server` (available as a test binary) +
/// pre-staged model files, so no live upstream engine is needed.
/// (TEST-28 — the crash/backoff SUPERVISION path — is implemented separately
/// below using a never-healthy binary that trips the flap cap.)
#[tokio::test]
async fn test_30_activate_drains_and_respawns_running_instance() {
    use super::{make_wav, stage_model, stub_whisper_binary};

    let server = TestServer::start().await;
    // Register the stub as the system-default runtime + stage two models on disk.
    let stub = stub_whisper_binary();
    insert_version_row(&server, "v0.0.0-stub", "cpu", stub.to_string_lossy().as_ref(), true).await;
    stage_model(&server, "base");
    stage_model(&server, "small");
    let admin = create_user_with_permissions(&server, "voice_mm_t30", VOICE_ADMIN_PERMS).await;

    // Register the two staged models as library rows so we can activate by id.
    // (Uploads would rewrite the files; here we insert rows matching the staged
    //  ggml-<name>.bin files directly via a re-upload of matching bytes is not
    //  needed — activate only needs a row whose `name` maps to an on-disk file.)
    // Simplest: upload the two models through the API so rows + files both exist.
    let mut base_bytes = b"ggml".to_vec();
    base_bytes.extend_from_slice(b"base-model-body");
    let mut small_bytes = b"ggml".to_vec();
    small_bytes.extend_from_slice(b"small-model-body");
    upload_model(&server, &admin.token, Some("base"), Some(("base.bin", base_bytes))).await;
    let id_small = upload_model(&server, &admin.token, Some("small"), Some(("small.bin", small_bytes)))
        .await
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Activate "base" and auto-start the instance via a transcribe.
    // (Default settings.model is already "base"; a transcribe boots it.)
    let part = reqwest::multipart::Part::bytes(make_wav(1.0))
        .file_name("a.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = client()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "transcribe auto-starts the instance");
    let (_, info) = get_json(&server, &admin.token, "/voice/instance").await;
    assert_eq!(info["status"], "running", "instance running after auto-start");
    assert_eq!(info["active_model"], "ggml-base.bin");

    // Activate "small" → the running instance is DRAINED + stopped (the switch is
    // applied lazily: the next transcribe respawns on the new model).
    let res = client()
        .post(server.api_url(&format!("/voice/models/{id_small}/activate")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "activate should 200");

    // settings.model reflects the activation immediately.
    let (_, settings) = get_json(&server, &admin.token, "/voice/settings").await;
    assert_eq!(settings["model"], "small", "activate updates settings.model");

    // Bounded poll: the running instance drains to `stopped` (not eagerly
    // respawned) after the model switch.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let (_, info) = get_json(&server, &admin.token, "/voice/instance").await;
        if info["status"] == "stopped" {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "running instance was not drained after activate (last status: {})",
                info["status"]
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // A fresh transcribe respawns the instance on the newly-activated model.
    let part = reqwest::multipart::Part::bytes(make_wav(1.0))
        .file_name("b.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = client()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "transcribe respawns on the new model");
    let (_, info) = get_json(&server, &admin.token, "/voice/instance").await;
    assert_eq!(info["status"], "running", "respawned after transcribe");
    assert_eq!(
        info["active_model"], "ggml-small.bin",
        "respawn serves the newly-activated model"
    );

    // Cleanup: stop the instance so the process is reaped promptly.
    let _ = client()
        .post(server.api_url("/voice/instance/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await;
}

/// TEST-28 — request-path crash supervision (F1/F2): a whisper-server binary that
/// never becomes healthy makes every auto-start time out → `Crashed` on each
/// transcribe; 5 crashes in the flap window latch the instance `failed` and stop
/// auto-respawning. Exercises the request-path wiring end-to-end (the health
/// state-machine transitions themselves are additionally unit-tested).
#[tokio::test]
async fn test_28_flap_cap_marks_failed_on_repeated_crash() {
    use super::{make_wav, stage_model};
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;

    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t28", VOICE_ADMIN_PERMS).await;

    // A "whisper-server" that spawns but never binds its port → health never
    // passes → each auto-start times out → Crashed.
    let dir = std::env::temp_dir().join(format!("voice-t28-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("never-healthy.sh");
    {
        let mut f = std::fs::File::create(&bin).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "sleep 30").unwrap();
    }
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    insert_version_row(&server, "v0.0.0-nohealth", "cpu", bin.to_string_lossy().as_ref(), true).await;
    stage_model(&server, "base");

    // Short auto-start timeout so 5 crashes happen in seconds, not minutes.
    let r = client()
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "auto_start_timeout_secs": 1, "model": "base" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "settings update should 200");

    let wav = make_wav(1.0);
    let mut failed = false;
    for _ in 0..8 {
        let part = reqwest::multipart::Part::bytes(wav.clone())
            .file_name("a.wav")
            .mime_str("audio/wav")
            .unwrap();
        let form = reqwest::multipart::Form::new().part("file", part);
        let _ = client()
            .post(server.api_url("/voice/transcribe"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .multipart(form)
            .send()
            .await
            .unwrap();
        let (_, info) = get_json(&server, &admin.token, "/voice/instance").await;
        if info["state"] == "failed" {
            failed = true;
            break;
        }
    }
    assert!(failed, "flap cap should latch the instance `failed` after repeated crashes");

    // While failed, a further transcribe is refused (flap protection), not respawned.
    let part = reqwest::multipart::Part::bytes(wav.clone())
        .file_name("a.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = client()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_ne!(resp.status(), 200, "a failed runtime must not auto-respawn");

    // Admin restart clears the failed latch (the endpoint runs; it may itself
    // fail health again, but clear_failed must have executed).
    let r = client()
        .post(server.api_url("/voice/instance/restart"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert!(r.status() != 403 && r.status() != 404, "restart endpoint reachable");

    std::fs::remove_dir_all(&dir).ok();
}

/// TEST-33 — instance logs endpoints (F8): admin gets 200 + a `lines` array
/// (empty when nothing is running); a non-admin is 403.
#[tokio::test]
async fn test_33_instance_logs_endpoints() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_mm_t33", VOICE_ADMIN_PERMS).await;
    let (st, v) = get_json(&server, &admin.token, "/voice/instance/logs").await;
    assert_eq!(st, StatusCode::OK, "admin can read logs");
    assert!(v["lines"].is_array(), "logs returns a lines array");

    let member =
        create_user_with_permissions(&server, "voice_mm_t33m", &["voice::transcribe"]).await;
    let r = client()
        .get(server.api_url("/voice/instance/logs"))
        .header("Authorization", format!("Bearer {}", member.token))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "logs needs voice::admin::read");
}
