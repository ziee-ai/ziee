//! In-test mock of the ziee-ai/hub GitHub **Pages** branch.
//!
//! Lets the hub integration tests exercise the refresh → parse →
//! lazy-fetch-manifest path WITHOUT touching the network. The spawned
//! ziee server is pointed at this mock via `ZIEE_HUB_PAGES_BASE`, the
//! debug-only override that's compiled out of release builds.
//!
//! Serves the Pages layout:
//!   GET /index.json                            → the Catalog
//!   GET /<folder>/<id>/<version>.json          → per-entry manifest
//!
//! `<folder>` is `models` / `assistants` / `mcp-servers` to match the
//! production layout (and `is_safe_manifest_path` validator).
//!
//! Tests that want to simulate a publisher updating the catalog can
//! call [`MockHub::switch_to`] to flip which `MockVersion` is the
//! active "published" state, then trigger another `/hub/refresh` on
//! the server side.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use serde_json::{json, Value as Json};

/// One catalog item to bake into a mock catalog version.
pub struct MockItem {
    pub category: &'static str, // "model" | "assistant" | "mcp-server"
    /// Reverse-DNS `name` (e.g. `"io.github.test/mock-asst-a"`). Must
    /// contain exactly one `/` — this is the catalog lookup key + the
    /// path layout under dist/`<category>/<namespace>/<leaf>/<version>.json`.
    pub name: &'static str,
    pub min_ziee_version: Option<&'static str>,
    /// Optional extra JSON fields merged into the generated manifest
    /// body. Use for tests that need fields the minimal manifest
    /// doesn't ship. `None` for most tests.
    pub extra_json: Option<Json>,
    /// For `mcp-server` items only: when true, emit a `remotes[]` with
    /// `type: streamable-http` instead of the default `packages[]`
    /// with `runtimeHint: npx`. Needed for tests that install on the
    /// user-scoped endpoint (the MCP user policy gates stdio whenever
    /// `code_sandbox.enabled` is false). Ignored for non-mcp-server.
    pub mcp_http: bool,
}

/// One mock catalog "version" (snapshot of what the Pages branch is
/// serving). Pages serves just one `index.json` at a time, so
/// `MockHub::switch_to` rotates which version is published.
pub struct MockVersion {
    pub version: &'static str, // e.g. "9.9.1-test" (no leading v)
    /// Retained for source-compat with legacy callers; ignored
    /// (no release list to flag).
    pub prerelease: bool,
    pub items: Vec<MockItem>,
}

/// In-memory representation of a built mock version — pre-rendered
/// index.json + per-entry manifest map keyed by `manifest_path`
/// (e.g. `"models/foo/1.0.0.json"`).
#[derive(Clone)]
struct PreparedCatalog {
    version: String,
    index_bytes: Vec<u8>,
    manifests: HashMap<String, Vec<u8>>,
}

pub struct MockHub {
    pub base_url: String,
    state: Arc<MockState>,
    _handle: tokio::task::JoinHandle<()>,
}

impl MockHub {
    /// Extra env to inject into a spawned TestServer so its HubManager
    /// fetches from this mock instead of GitHub Pages.
    pub fn test_env(&self) -> Vec<(String, String)> {
        vec![("ZIEE_HUB_PAGES_BASE".into(), self.base_url.clone())]
    }

    /// Flip the served catalog to a different prepared version. Used
    /// by tests that want to simulate a publisher pushing a newer
    /// `index.json` between two `/hub/refresh` calls. Panics if the
    /// version string doesn't match any registered version.
    pub fn switch_to(&self, version: &str) {
        let mut active = self.state.active.lock().expect("mock state poisoned");
        let prepared = self
            .state
            .prepared
            .get(version)
            .unwrap_or_else(|| panic!("mock hub has no prepared version {version:?}"));
        *active = prepared.clone();
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

/// Build the JSON body for one per-entry manifest. Mirrors the shape
/// the hub-seed manifests use so the typed `HubModel` /
/// `HubAssistant` / `HubMCPServer` structs deserialize cleanly.
///
/// `name` is reverse-DNS (e.g. `io.github.test/foo`); the leaf
/// (after the last `/`) is used for display-fallback labels in
/// model/assistant manifests.
fn minimal_manifest_for(category: &str, name: &str, mcp_http: bool) -> Json {
    let leaf = name.rsplit('/').next().unwrap_or(name);
    match category {
        // Body shape — `sources[]` carries the per-source registry /
        // file format / quantizations (no flat top-level fields).
        "model" => json!({
            "name": name,
            "display_name": leaf,
            "version": "1.0.0",
            "sources": [{
                "registryType": "huggingface",
                "identifier": format!("test/{leaf}"),
                "version": "main",
                "fileFormat": "safetensors",
                "quantizations": [{
                    "name": "f16",
                    "mainFile": "model.safetensors",
                    "sizeGb": 1.0,
                    "isDefault": true
                }]
            }],
            "tags": ["mock"],
        }),
        "assistant" => json!({
            "name": name,
            "display_name": leaf,
            "version": "1.0.0",
            "parameters": {},
        }),
        // Strict server.json `remotes[]` for HTTP mocks.
        "mcp-server" if mcp_http => json!({
            "name": name,
            "description": format!("mock {leaf}"),
            "version": "1.0.0",
            "remotes": [{
                "type": "streamable-http",
                "url": "https://example.com/mcp",
                "headers": []
            }],
        }),
        // Strict server.json `packages[]` for stdio mocks. `npx` is in
        // `HOST_ALLOWED_COMMANDS` so host (non-sandbox) installs pass
        // the command-validation tier.
        "mcp-server" => json!({
            "name": name,
            "description": format!("mock {leaf}"),
            "version": "1.0.0",
            "packages": [{
                "registryType": "npm",
                "identifier": leaf,
                "version": "1.0.0",
                "transport": { "type": "stdio" },
                "runtimeHint": "npx",
                "runtimeArguments": [],
                "packageArguments": [],
                "environmentVariables": []
            }],
        }),
        _ => json!({"name": name}),
    }
}

fn merge_into(base: &mut Json, extra: Json) {
    let (Json::Object(base), Json::Object(extra)) = (base, extra) else {
        return;
    };
    for (k, v) in extra {
        base.insert(k, v);
    }
}

fn prepare_catalog(v: &MockVersion) -> PreparedCatalog {
    let _ = v.prerelease; // no release list; field kept for source-compat.

    let mut manifests: HashMap<String, Vec<u8>> = HashMap::new();
    let mut index_items: Vec<Json> = Vec::new();

    for it in &v.items {
        // Path layout: `<folder>/<namespace>/<leaf>/<version>.json`
        // — split on the FIRST `/`. Panics in test if name lacks `/`.
        let (namespace, leaf) = it
            .name
            .split_once('/')
            .unwrap_or_else(|| panic!("MockItem.name must be reverse-DNS with one `/`: {:?}", it.name));
        let manifest_path = format!(
            "{}/{}/{}/{}.json",
            folder(it.category),
            namespace,
            leaf,
            v.version
        );
        let mut body = minimal_manifest_for(it.category, it.name, it.mcp_http);
        if let Some(extra) = it.extra_json.clone() {
            merge_into(&mut body, extra);
        }
        manifests.insert(
            manifest_path.clone(),
            serde_json::to_vec(&body).expect("serialize manifest"),
        );

        let item = json!({
            "name": it.name,
            "category": it.category,
            "title": leaf,
            "summary": format!("mock {}", leaf),
            "tags": ["mock"],
            "verified": true,
            "added_at": "2026-05-29T00:00:00Z",
            "min_ziee_version": it.min_ziee_version,
            "manifest_path": manifest_path,
            "version": v.version,
        });
        index_items.push(item);
    }

    let index = json!({
        "schema_version": 2,
        "hub_version": v.version,
        "generated_at": "2026-05-29T00:00:00Z",
        "items": index_items,
    });

    PreparedCatalog {
        version: v.version.to_string(),
        index_bytes: serde_json::to_vec(&index).expect("serialize index"),
        manifests,
    }
}

struct MockState {
    /// `version_string -> PreparedCatalog` for every registered version,
    /// looked up by `switch_to`.
    prepared: HashMap<String, PreparedCatalog>,
    /// Catalog currently being served from `/index.json` and the
    /// per-entry endpoints. Replaced wholesale by `switch_to`.
    active: Mutex<PreparedCatalog>,
}

async fn serve_index(State(state): State<Arc<MockState>>) -> Response {
    let active = state.active.lock().expect("mock state poisoned");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(active.index_bytes.clone()))
        .unwrap()
}

async fn serve_manifest(
    State(state): State<Arc<MockState>>,
    Path(rest): Path<String>,
) -> Response {
    let active = state.active.lock().expect("mock state poisoned");
    match active.manifests.get(&rest) {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes.clone()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

/// Build + start the mock. The FIRST version in the list is the
/// initially-active catalog (matches v1's "newest-first" convention —
/// tests that want to "activate" the older version flip with
/// `switch_to`); every version is pre-rendered so any of them can be
/// served later. Most tests only register one version + never switch.
pub async fn spawn_mock_hub(versions: Vec<MockVersion>) -> MockHub {
    assert!(!versions.is_empty(), "spawn_mock_hub needs at least one version");

    let mut prepared: HashMap<String, PreparedCatalog> = HashMap::new();
    let mut first: Option<PreparedCatalog> = None;
    for v in &versions {
        let cat = prepare_catalog(v);
        if first.is_none() {
            first = Some(cat.clone());
        }
        prepared.insert(cat.version.clone(), cat);
    }

    let state = Arc::new(MockState {
        prepared,
        active: Mutex::new(first.expect("at least one prepared catalog")),
    });

    let app = Router::new()
        .route("/index.json", get(serve_index))
        .route("/{*rest}", get(serve_manifest))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    MockHub {
        base_url: format!("http://127.0.0.1:{}", addr.port()),
        state,
        _handle: handle,
    }
}
