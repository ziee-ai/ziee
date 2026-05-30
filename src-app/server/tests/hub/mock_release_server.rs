//! In-test mock of the ziee-ai/hub GitHub Releases surface.
//!
//! Lets the hub integration tests exercise the full
//! fetch → sha256 → unpack → rotate path WITHOUT touching the network
//! or needing a real cosign signature. The spawned ziee server is
//! pointed at this mock via the debug-only overrides
//! (`ZIEE_HUB_API_BASE_OVERRIDE`, `ZIEE_HUB_DOWNLOAD_BASE_OVERRIDE`,
//! `ZIEE_HUB_ALLOW_UNSIGNED=1`), which are compiled out of release
//! builds.
//!
//! Serves, for each configured version:
//!   GET /repos/ziee-ai/hub/releases                          → release list JSON
//!   GET /ziee-ai/hub/releases/download/<tag>/hub.tar.gz       → manifest bundle
//!   GET /ziee-ai/hub/releases/download/<tag>/hub.tar.gz.sha256
//!   GET /ziee-ai/hub/releases/download/<tag>/hub.index.json
//!   GET /ziee-ai/hub/releases/download/<tag>/hub.index.json.sha256

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use sha2::{Digest, Sha256};

/// One catalog item to bake into a mock version.
pub struct MockItem {
    pub category: &'static str, // "model" | "assistant" | "mcp-server"
    pub id: &'static str,
    pub min_ziee_version: Option<&'static str>,
}

/// One mock release version.
pub struct MockVersion {
    pub version: &'static str, // e.g. "9.9.1-test" (no leading v)
    pub prerelease: bool,
    pub items: Vec<MockItem>,
}

pub struct MockHub {
    pub base_url: String,
    _handle: tokio::task::JoinHandle<()>,
}

impl MockHub {
    /// Extra env to inject into a spawned TestServer so its HubManager
    /// fetches from this mock with cosign skipped.
    pub fn test_env(&self) -> Vec<(String, String)> {
        vec![
            ("ZIEE_HUB_API_BASE_OVERRIDE".into(), self.base_url.clone()),
            ("ZIEE_HUB_DOWNLOAD_BASE_OVERRIDE".into(), self.base_url.clone()),
            ("ZIEE_HUB_ALLOW_UNSIGNED".into(), "1".into()),
        ]
    }
}

fn folder(category: &str) -> &'static str {
    match category {
        "model" => "models",
        "assistant" => "assistants",
        "mcp-server" => "mcp-servers",
        _ => "models",
    }
}

fn minimal_manifest(category: &str, id: &str) -> String {
    match category {
        "model" => format!(
            "id: {id}\nname: {id}\ndisplay_name: {id}\nrepository_url: https://huggingface.co\nrepository_path: test/{id}\nmain_filename: model.safetensors\nfile_format: safetensors\nsize_gb: 1.0\npopularity_score: 0.5\n"
        ),
        "assistant" => format!(
            "id: {id}\nname: {id}\ndisplay_name: {id}\nparameters: {{}}\n"
        ),
        _ => format!("id: {id}\nname: {id}\ndisplay_name: {id}\n"),
    }
}

fn build_index(v: &MockVersion) -> String {
    let items: Vec<String> = v
        .items
        .iter()
        .map(|it| {
            let min = it
                .min_ziee_version
                .map(|m| format!("\"{m}\""))
                .unwrap_or_else(|| "null".to_string());
            format!(
                r#"    {{"id":"{id}","category":"{cat}","name":"{id}","summary":"mock {id}","tags":["mock"],"verified":true,"added_at":"2026-05-29","min_ziee_version":{min},"manifest_path":"{folder}/{id}.yaml"}}"#,
                id = it.id,
                cat = it.category,
                folder = folder(it.category),
                min = min,
            )
        })
        .collect();
    format!(
        "{{\n  \"schema_version\": 1,\n  \"hub_version\": \"{ver}\",\n  \"generated_at\": \"2026-05-29T00:00:00Z\",\n  \"items\": [\n{items}\n  ]\n}}\n",
        ver = v.version,
        items = items.join(",\n"),
    )
}

fn build_tarball(v: &MockVersion, index_json: &str) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    // manifests
    for it in &v.items {
        let path = format!("{}/{}.yaml", folder(it.category), it.id);
        let body = minimal_manifest(it.category, it.id);
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, path, body.as_bytes()).unwrap();
    }
    // index.json at root (gets overwritten by the verified copy, but
    // present so the structure matches a real bundle)
    let mut header = tar::Header::new_gnu();
    header.set_size(index_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "index.json", index_json.as_bytes())
        .unwrap();
    let tar_bytes = tar.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&tar_bytes).unwrap();
    gz.finish().unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

struct MockState {
    /// path → (content-type, bytes)
    routes: HashMap<String, (String, Vec<u8>)>,
}

async fn serve_asset(
    State(state): State<Arc<MockState>>,
    Path(rest): Path<String>,
) -> Response {
    let key = format!("/{}", rest);
    match state.routes.get(&key) {
        Some((ct, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, ct)
            .body(Body::from(bytes.clone()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

/// Build + start the mock. Versions are listed newest-first in the
/// release JSON (GitHub's order).
pub async fn spawn_mock_hub(versions: Vec<MockVersion>) -> MockHub {
    let mut routes: HashMap<String, (String, Vec<u8>)> = HashMap::new();

    // Release list JSON.
    let releases: Vec<String> = versions
        .iter()
        .map(|v| {
            format!(
                r#"{{"tag_name":"v{ver}","prerelease":{pre},"draft":false,"published_at":"2026-05-29T00:00:00Z"}}"#,
                ver = v.version,
                pre = v.prerelease,
            )
        })
        .collect();
    routes.insert(
        "/repos/ziee-ai/hub/releases".to_string(),
        (
            "application/json".to_string(),
            format!("[{}]", releases.join(",")).into_bytes(),
        ),
    );

    // Per-version download assets.
    for v in &versions {
        let index = build_index(v);
        let tarball = build_tarball(v, &index);
        let tag = format!("v{}", v.version);
        let base = format!("/ziee-ai/hub/releases/download/{}", tag);

        routes.insert(
            format!("{base}/hub.index.json"),
            ("application/json".into(), index.clone().into_bytes()),
        );
        routes.insert(
            format!("{base}/hub.index.json.sha256"),
            (
                "text/plain".into(),
                format!("{}  hub.index.json\n", sha256_hex(index.as_bytes())).into_bytes(),
            ),
        );
        routes.insert(
            format!("{base}/hub.tar.gz"),
            ("application/gzip".into(), tarball.clone()),
        );
        routes.insert(
            format!("{base}/hub.tar.gz.sha256"),
            (
                "text/plain".into(),
                format!("{}  hub.tar.gz\n", sha256_hex(&tarball)).into_bytes(),
            ),
        );
    }

    let state = Arc::new(MockState { routes });
    let app = Router::new()
        .route("/{*rest}", get(serve_asset))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    MockHub {
        base_url: format!("http://127.0.0.1:{}", addr.port()),
        _handle: handle,
    }
}
