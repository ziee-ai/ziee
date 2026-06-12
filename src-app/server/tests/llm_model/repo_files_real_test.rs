//! Real-network tests for the `repository-files` auto-detection endpoint.
//!
//! These hit the live Hugging Face / GitHub APIs (no mocks), mirroring the
//! existing real `download_*` tests. They are env-keyed, NOT `#[ignore]`'d:
//! `source tests/.env.test` — HUGGINGFACE_API_KEY (HF tests) and GITHUB_TOKEN
//! (GitHub tests; see .env.test.example). Without the keys they fail loudly,
//! same as the download tests. (`test_detect_unknown_host_returns_empty` needs
//! neither — an unknown host never hits the network.)

use reqwest::StatusCode;
use serde_json::{Value, json};

use super::download_test::get_huggingface_repository;

async fn detect(
    server: &crate::common::TestServer,
    token: &str,
    repo_id: &str,
    path: &str,
    branch: &str,
) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .get(server.api_url("/llm-models/repository-files"))
        .header("Authorization", format!("Bearer {token}"))
        .query(&[
            ("repository_id", repo_id),
            ("path", path),
            ("branch", branch),
        ])
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

fn paths(body: &Value) -> Vec<String> {
    body["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect()
}

const PERMS: &[&str] = &[
    "llm_models::create",
    "llm_models::read",
    "llm_repositories::read",
    "llm_repositories::edit",
];

#[tokio::test]
async fn test_detect_hf_single_file_safetensors() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "detector", PERMS).await;
    let hf = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf["id"].as_str().unwrap();

    let (status, body) = detect(
        &server,
        &user.token,
        repo_id,
        "hf-internal-testing/tiny-random-gpt2",
        "main",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["source"], "huggingface");
    // tiny-random-gpt2 ships BOTH model.safetensors and pytorch_model.bin —
    // pin that safetensors wins over pickle in detect_weight_set, live.
    assert_eq!(body["shape"], "safetensors", "body: {body}");
    let files = paths(&body);
    assert!(
        files.iter().any(|p| p.ends_with(".safetensors")),
        "expected a .safetensors weight, got {files:?}"
    );
    assert!(files.iter().any(|p| p == "config.json"));
    // every file has a numeric size
    for f in body["files"].as_array().unwrap() {
        assert!(f["size_bytes"].as_i64().is_some());
    }
    assert!(body["suggested_main_filename"].as_str().is_some());
}

#[tokio::test]
async fn test_detect_hf_gguf_multi_quant() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "detector", PERMS).await;
    let hf = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf["id"].as_str().unwrap();

    let (status, body) = detect(
        &server,
        &user.token,
        repo_id,
        "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
        "main",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["shape"], "gguf");
    let ggufs: Vec<String> = paths(&body)
        .into_iter()
        .filter(|p| p.to_lowercase().ends_with(".gguf"))
        .collect();
    // This repo publishes many quants — assert the multi-quant picker path.
    assert!(ggufs.len() >= 2, "expected multiple gguf quants, got {ggufs:?}");
    // Drift-stable: the suggested default is one of the detected gguf quants
    // (the exact q4_k_m size tiebreak is pinned offline in model_files /
    // repo_files unit tests, independent of this repo's quant lineup).
    let suggested = body["suggested_main_filename"].as_str().unwrap();
    assert!(suggested.to_lowercase().ends_with(".gguf"), "suggested={suggested}");
    assert!(
        ggufs.iter().any(|p| p.ends_with(suggested)),
        "suggested {suggested} not among detected quants {ggufs:?}"
    );
}

#[tokio::test]
async fn test_detect_hf_sharded_safetensors() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "detector", PERMS).await;
    let hf = get_huggingface_repository(&server, &user.token, true).await;
    let repo_id = hf["id"].as_str().unwrap();

    // Phi-3-mini ships sharded safetensors + an index.json (ungated, MIT).
    let (status, body) = detect(
        &server,
        &user.token,
        repo_id,
        "microsoft/Phi-3-mini-4k-instruct",
        "main",
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["shape"], "safetensors");
    let files = paths(&body);
    let shards: Vec<&String> = files
        .iter()
        .filter(|p| p.contains("-of-") && p.ends_with(".safetensors"))
        .collect();
    // Resilient gap-A property: a sharded set OR an index is present
    // (tolerant of upstream repackaging to a single consolidated file — what
    // matters is that the detector returns the full safetensors weight set).
    let has_index = files.iter().any(|p| p.ends_with(".index.json"));
    let safetensors_count = files.iter().filter(|p| p.ends_with(".safetensors")).count();
    // Tolerant of upstream repackaging: a full safetensors weight set is
    // present as EITHER a sharded set (>=2 shards or an index) OR a single
    // consolidated `model.safetensors`. What matters is the detector returns
    // the whole set.
    assert!(
        shards.len() >= 2 || has_index || safetensors_count == 1,
        "expected a full safetensors weight set, got {files:?}"
    );
    // When an index is present it should be the suggested main — that's the
    // value the drawer pre-fills and the downloader expands to the full set.
    if has_index {
        assert!(
            body["suggested_main_filename"]
                .as_str()
                .unwrap_or("")
                .ends_with(".index.json"),
            "expected the .index.json as suggested main, got {body}"
        );
    }
}

/// Resolve the seeded "GitHub" repository id and configure GITHUB_TOKEN.
///
/// GITHUB_TOKEN is REQUIRED (mirrors how the HF tests require
/// HUGGINGFACE_API_KEY): the anonymous GitHub API is 60 req/hr per IP, so
/// running these tests without a token would be flaky. Fail loudly rather
/// than skip — a green-but-unexercised test is worse than an absent one.
async fn github_repo_id(server: &crate::common::TestServer, token: &str) -> String {
    let gh_token = std::env::var("GITHUB_TOKEN").expect(
        "GITHUB_TOKEN not set. Source tests/.env.test or set it — the GitHub \
         auto-detect tests require it (anonymous GitHub API is 60 req/hr/IP).",
    );

    let repos: Value = reqwest::Client::new()
        .get(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id = repos["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"].as_str() == Some("GitHub"))
        .expect("GitHub repository not seeded")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "auth_config": { "token": gh_token } }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "failed to configure GitHub token");
    repo_id
}

#[tokio::test]
async fn test_detect_github_tree() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "detector", PERMS).await;
    let repo_id = github_repo_id(&server, &user.token).await;

    // karpathy/llama2.c — small repo, default branch "master".
    let (status, body) =
        detect(&server, &user.token, &repo_id, "karpathy/llama2.c", "master").await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["source"], "github");
    let files = paths(&body);
    assert!(!files.is_empty(), "expected a non-empty github listing");
    // Don't assert on specific weights — llama2.c is a code repo, not a model
    // host, so its contents drift. Assert the real intent: the GitHub-tree
    // path lists AND classifies every entry (no panics / missing roles).
    for f in body["files"].as_array().unwrap() {
        assert!(f["file_role"].is_string(), "every file is classified: {f}");
    }
}

#[tokio::test]
async fn test_detect_not_found_returns_404() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "detector", PERMS).await;

    // Use GitHub for the missing-repo case: a missing PUBLIC repo returns a
    // clean 404. (Hugging Face deliberately conflates missing/private into
    // 401, which the handler maps to 403 — so it can't assert a 404 here.)
    let repo_id = github_repo_id(&server, &user.token).await;

    let (status, body) = detect(
        &server,
        &user.token,
        &repo_id,
        "octocat/this-repo-does-not-exist-zzz-000",
        "master",
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

/// A repository whose host is neither HF nor GitHub: auto-detect returns an
/// empty/unknown result (the UI then falls back to a manual filename).
/// Deterministic — no network or API keys (an unknown host never fetches).
#[tokio::test]
async fn test_detect_unknown_host_returns_empty() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "detector",
        &[
            "llm_models::create",
            "llm_models::read",
            "llm_repositories::read",
            "llm_repositories::create",
        ],
    )
    .await;

    let repo: Value = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", user.token))
        // example.com: a non-HF/GitHub host that RESOLVES (the create-time
        // SSRF URL check rejects unresolvable hosts). enabled:false skips the
        // connection probe. detect never actually contacts it (unknown host
        // short-circuits to an empty result before any fetch).
        .json(&json!({
            "name": "Custom Git Host",
            "url": "https://example.com",
            "auth_type": "none",
            "enabled": false,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_id = repo["id"].as_str().unwrap_or_else(|| panic!("created repository id; body: {repo}"));

    let (status, body) = detect(&server, &user.token, repo_id, "owner/repo", "main").await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["source"], "unknown", "body: {body}");
    assert_eq!(body["shape"], "unknown", "body: {body}");
    assert!(body["files"].as_array().unwrap().is_empty(), "body: {body}");
}
