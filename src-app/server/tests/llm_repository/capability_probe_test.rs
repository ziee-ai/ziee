//! Capability-probe tests — ITEM-11 / ITEM-12, pinning INV-4.
//!
//! The defect these exist for was reproduced against a LIVE server: a Vite
//! dev server (`http://127.0.0.1:<port>/models`), `https://api.github.com`
//! and `https://huggingface.co/custom` were all reported `healthy` by
//! `POST /api/llm-repositories/{id}/test`, because the probe asserted only
//! `status == 200` and never read the body.
//!
//! INV-4: a repository is reported `healthy` only when a model-serving
//! capability was positively confirmed; reachability alone is never
//! `healthy`, and a repository whose capability could not be confirmed is
//! never auto-disabled.
//!
//! Every rejection assertion below ships with its happy-path counterpart in
//! the SAME test, so a rejection that passes because the endpoint broke some
//! other way cannot go unnoticed.

use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::create_user_with_permissions;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const REPO_ADMIN_PERMS: &[&str] = &[
    "llm_repositories::read",
    "llm_repositories::create",
    "llm_repositories::edit",
    "llm_repositories::delete",
];

/// A trimmed but faithful Hugging Face `/api/models` payload — a JSON array
/// of model records. This is the shape the probe now requires before it will
/// say `healthy`.
const HF_MODEL_LISTING: &str = r#"[
    {
        "_id": "621ffdc036468d709f17434d",
        "id": "openai-community/gpt2",
        "modelId": "openai-community/gpt2",
        "likes": 2743,
        "private": false,
        "downloads": 12345678,
        "tags": ["transformers", "gpt2"],
        "pipeline_tag": "text-generation"
    }
]"#;

/// Aborts the fixture's serving task on scope exit — success OR panic-unwind —
/// so a failed assertion cannot leak a spawned listener.
struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Bind a loopback fixture and return `(base_url, guard)`.
async fn spawn_fixture(app: Router) -> (String, AbortOnDrop) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let guard = AbortOnDrop(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));
    (format!("http://127.0.0.1:{}", addr.port()), guard)
}

/// The reported defect, reproduced: a dev server whose SPA fallback answers
/// **200 to every GET** with an HTML document. Reachable; serves no models.
async fn spa_fallback_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(Router::new().fallback(get(|| async {
        (
            StatusCode::OK,
            [("content-type", "text/html")],
            "<!doctype html><html><body><div id=\"root\"></div></body></html>",
        )
            .into_response()
    })))
    .await
}

/// A real model registry: `/api/models` serves a model listing.
async fn model_registry_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(
        Router::new()
            .route(
                "/api/models",
                get(|| async {
                    (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        HF_MODEL_LISTING,
                    )
                        .into_response()
                }),
            )
            .fallback(get(|| async { "ok" })),
    )
    .await
}

/// The org a row must name for [`author_aware_registry_fixture`] to list any
/// models. Any other org is, as far as that fixture is concerned, nonexistent.
const KNOWN_ORG: &str = "acme-models";

/// A model registry that answers the `author=` filter HONESTLY — a listing for
/// [`KNOWN_ORG`], an empty array for every other org.
///
/// This is what makes the defect-2 assertions mean anything. The older
/// `model_registry_fixture` serves the same listing for every query, so a row
/// naming a NONEXISTENT org still reads `healthy` against it — which is exactly
/// the bug (`capability_probe_url` graded Hugging Face's global catalogue, never
/// the row). Only an author-aware fixture can tell the two apart.
async fn author_aware_registry_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(
        Router::new()
            .route(
                "/api/models",
                get(
                    |axum::extract::RawQuery(q): axum::extract::RawQuery| async move {
                        let query = q.unwrap_or_default();
                        let author = query.split('&').find_map(|kv| kv.strip_prefix("author="));
                        let body = match author {
                            // No author filter: the global catalogue, as today.
                            None => HF_MODEL_LISTING,
                            Some(a) if a == KNOWN_ORG => HF_MODEL_LISTING,
                            // A real Hugging Face answers an unknown author with an
                            // empty array, not a 404.
                            Some(_) => "[]",
                        };
                        (StatusCode::OK, [("content-type", "application/json")], body)
                            .into_response()
                    },
                ),
            )
            .fallback(get(|| async { "ok" })),
    )
    .await
}

/// Reachable, but there is no model listing here at all — the shape a
/// self-hosted service that simply is not a model registry has. Must be
/// `unverified`, NOT `unhealthy`, or shipping this change would auto-disable
/// working deployments.
async fn no_listing_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(
        Router::new()
            .route("/", get(|| async { "service up" }))
            .fallback(get(|| async { (StatusCode::NOT_FOUND, "not found") })),
    )
    .await
}

/// Rejects everything with 401 — a stale/missing credential. A genuine
/// failure, which must stay `unhealthy` and keep auto-disabling.
async fn unauthorized_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(
        Router::new().fallback(get(|| async { (StatusCode::UNAUTHORIZED, "Unauthorized") })),
    )
    .await
}

/// 200 + valid JSON that is NOT a model listing — proves the check is a
/// SHAPE assertion, not "did the body parse".
async fn valid_json_not_a_listing_fixture() -> (String, AbortOnDrop) {
    spawn_fixture(Router::new().fallback(get(|| async {
        (
            StatusCode::OK,
            [("content-type", "application/json")],
            r#"{"ok": true, "service": "definitely not a model registry"}"#,
        )
            .into_response()
    })))
    .await
}

async fn pool_for(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test pool")
}

/// `(enabled, last_health_check_status, last_health_check_reason)`
async fn read_health(pool: &sqlx::PgPool, repo_id: Uuid) -> (bool, String, Option<String>) {
    let row = sqlx::query!(
        r#"SELECT enabled, last_health_check_status, last_health_check_reason
           FROM llm_repositories WHERE id = $1"#,
        repo_id,
    )
    .fetch_one(pool)
    .await
    .expect("read repo row");
    (
        row.enabled,
        row.last_health_check_status,
        row.last_health_check_reason,
    )
}

/// Create an ENABLED repository (so `enforce_on_create` probes it) and
/// return its id.
async fn create_enabled_repo(server: &TestServer, token: &str, name: &str, url: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": name,
            "url": url,
            "auth_type": "none",
            "enabled": true,
        }))
        .send()
        .await
        .expect("create request failed");
    let status = res.status();
    let body: Value = res.json().await.expect("create response is JSON");
    assert_eq!(status, 201, "create should return 201; body={body}");
    Uuid::parse_str(body["id"].as_str().expect("id")).expect("uuid")
}

/// `POST /llm-repositories/{id}/test` with no form overrides.
async fn probe_by_id(server: &TestServer, token: &str, repo_id: Uuid) -> Value {
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({}))
        .send()
        .await
        .expect("probe request failed");
    assert_eq!(res.status(), 200);
    res.json().await.expect("probe response is JSON")
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-17 [acceptance] INV-4 — reachability alone is not `healthy`, and a
// real model listing IS. Both halves in one test.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn reachable_web_server_is_not_healthy_while_a_model_listing_is() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    // ── Half A: the reported defect. A dev server answering 200 to EVERY
    //    GET with an HTML SPA shell. This is the `http://127.0.0.1:<vite>/models`
    //    row that was displayed as `healthy`.
    let (spa_base, _spa_guard) = spa_fallback_fixture().await;
    let spa_id = create_enabled_repo(
        &server,
        &admin.token,
        "vite-dev-server",
        &format!("{spa_base}/models"),
    )
    .await;

    let spa_result = probe_by_id(&server, &admin.token, spa_id).await;
    assert_eq!(
        spa_result["success"], false,
        "a web server that answers 200 to everything must not pass the probe: {spa_result}"
    );
    let (spa_enabled, spa_status, spa_reason) = read_health(&pool, spa_id).await;
    assert_ne!(
        spa_status, "healthy",
        "INV-4: reachability alone must NEVER be recorded as healthy (reason: {spa_reason:?})"
    );
    assert_eq!(
        spa_status, "unverified",
        "a reachable host with no model listing is unverified"
    );
    assert!(
        spa_enabled,
        "INV-4: an unconfirmable repository must not be auto-disabled"
    );

    // ── Half B: the positive control. A host that actually serves a model
    //    listing IS healthy — so half A cannot be passing merely because the
    //    probe now rejects everything.
    let (registry_base, _registry_guard) = model_registry_fixture().await;
    let registry_id =
        create_enabled_repo(&server, &admin.token, "real-registry", &registry_base).await;

    let registry_result = probe_by_id(&server, &admin.token, registry_id).await;
    assert_eq!(
        registry_result["success"], true,
        "a host serving a model listing must pass: {registry_result}"
    );
    assert_eq!(registry_result["status"], "healthy");
    let (registry_enabled, registry_status, _) = read_health(&pool, registry_id).await;
    assert_eq!(registry_status, "healthy");
    assert!(registry_enabled);
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-18 [acceptance] INV-4 — `unverified` is a distinct outcome: it is
// recorded, it does NOT auto-disable, and a genuine failure still does.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn unverified_keeps_the_row_enabled_while_a_real_failure_auto_disables() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    // ── Half A: reachable, kind unclassifiable, no model listing.
    let (up_base, _up_guard) = no_listing_fixture().await;
    let unclassifiable_id =
        create_enabled_repo(&server, &admin.token, "self-hosted-thing", &up_base).await;

    let result = probe_by_id(&server, &admin.token, unclassifiable_id).await;
    assert_eq!(result["status"], "unverified", "outcome: {result}");
    assert_eq!(
        result["success"], false,
        "unverified must not be reported as a success"
    );
    let (enabled, status, reason) = read_health(&pool, unclassifiable_id).await;
    assert_eq!(status, "unverified");
    assert!(
        reason.is_some(),
        "unverified must record WHY it could not be confirmed"
    );
    assert!(
        enabled,
        "INV-4: a repository whose capability could not be confirmed must NEVER be auto-disabled"
    );

    // ── Half B: a genuine failure on the same shape of row still disables.
    //    Without this half, half A would also pass if auto-disable had simply
    //    been deleted.
    let (bad_base, _bad_guard) = unauthorized_fixture().await;
    let failing_id = create_enabled_repo(&server, &admin.token, "stale-token", &bad_base).await;
    // The create-flow probe already ran and auto-disabled it; re-enable so the
    // test-button path exercises the auto-disable transition explicitly.
    sqlx::query!(
        "UPDATE llm_repositories SET enabled = TRUE WHERE id = $1",
        failing_id
    )
    .execute(&pool)
    .await
    .expect("re-enable row");

    let bad_result = probe_by_id(&server, &admin.token, failing_id).await;
    assert_eq!(bad_result["status"], "unhealthy", "outcome: {bad_result}");
    assert_eq!(bad_result["success"], false);
    let (bad_enabled, bad_status, bad_reason) = read_health(&pool, failing_id).await;
    assert_eq!(bad_status, "unhealthy");
    assert!(bad_reason.is_some());
    assert!(
        !bad_enabled,
        "a genuine failure on an enabled row must still auto-disable"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-19 — the check is a SHAPE assertion, not "the body parsed as JSON".
//
// Halves A and B are fully offline (loopback fixtures) so the core assertion
// never depends on name resolution. Half C additionally drives the
// host-CLASSIFICATION path through the debug-only `LLM_REPOSITORY_HF_API_BASE`
// seam (DEC-14) — it needs `huggingface.co` to RESOLVE (never to be reached),
// the same assumption the suite's existing Hugging Face repository tests make.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn valid_json_that_is_not_a_model_listing_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    // ── Half A: 200 + parseable JSON that is not a model listing.
    let (bad_base, _bad_guard) = valid_json_not_a_listing_fixture().await;
    let bad_id = create_enabled_repo(&server, &admin.token, "json-not-listing", &bad_base).await;

    let result = probe_by_id(&server, &admin.token, bad_id).await;
    assert_eq!(
        result["status"], "unverified",
        "a body that merely PARSES is not evidence of a model listing: {result}"
    );
    let (enabled, status, _) = read_health(&pool, bad_id).await;
    assert_ne!(status, "healthy");
    assert!(enabled, "a shape mismatch must not auto-disable");

    // ── Half B: the same code path, a body that IS a listing. Without this
    //    half, half A would also pass if JSON parsing had simply broken.
    let (good_base, _good_guard) = model_registry_fixture().await;
    let good_id = create_enabled_repo(&server, &admin.token, "json-listing", &good_base).await;
    let good_result = probe_by_id(&server, &admin.token, good_id).await;
    assert_eq!(
        good_result["status"], "healthy",
        "the same path with a real listing must be healthy: {good_result}"
    );
}

#[tokio::test]
async fn a_hugging_face_host_is_probed_against_the_hugging_face_listing_surface() {
    // Drives the HOST-classification branch: the repository URL's host decides
    // WHICH listing surface is probed. The debug-only seam redirects that
    // surface at a loopback fixture, so no request leaves the machine — but
    // `huggingface.co` must still resolve for the create-time URL validation
    // (a DNS failure here is environmental, not a defect in the probe).
    let (bad_base, _bad_guard) = valid_json_not_a_listing_fixture().await;
    let (good_base, _good_guard) = model_registry_fixture().await;

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LLM_REPOSITORY_HF_API_BASE".to_string(), bad_base)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    let hf_id = create_enabled_repo(
        &server,
        &admin.token,
        "hf-shaped",
        &format!("https://huggingface.co/org-{}", Uuid::new_v4()),
    )
    .await;
    let result = probe_by_id(&server, &admin.token, hf_id).await;
    assert_eq!(
        result["status"], "unverified",
        "a Hugging Face host whose listing surface returns a non-listing must not \
         be healthy: {result}"
    );
    let (enabled, status, _) = read_health(&pool, hf_id).await;
    assert_ne!(status, "healthy");
    assert!(enabled);

    // Positive control on the SAME classification path: the seam now points at
    // a fixture that serves a real listing, and the row names an org that
    // fixture actually lists.
    //
    // CORRECTED (defect 2). This leg previously named
    // `https://huggingface.co/org-<random-uuid>` — a GUARANTEED-NONEXISTENT org
    // — and asserted `healthy`. That assertion did not test the positive
    // control it claimed to; it CERTIFIED the bug: `capability_probe_url`
    // discarded the row's URL and graded the fixture's global catalogue, so any
    // org name whatsoever passed. The org is now one the fixture really lists,
    // and the nonexistent-org case is asserted below as `unverified`.
    let good_server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LLM_REPOSITORY_HF_API_BASE".to_string(), good_base)],
        ..Default::default()
    })
    .await;
    let good_admin = create_user_with_permissions(&good_server, "admin", REPO_ADMIN_PERMS).await;
    let good_pool = pool_for(&good_server).await;
    let good_id = create_enabled_repo(
        &good_server,
        &good_admin.token,
        "hf-shaped-ok",
        &format!("https://huggingface.co/{KNOWN_ORG}"),
    )
    .await;
    let good_result = probe_by_id(&good_server, &good_admin.token, good_id).await;
    assert_eq!(
        good_result["status"], "healthy",
        "an org-scoped Hugging Face row whose org DOES list models must be \
         healthy: {good_result}"
    );
    let (good_enabled, good_status, _) = read_health(&good_pool, good_id).await;
    assert_eq!(good_status, "healthy");
    assert!(
        good_enabled,
        "INV-4: the happy path must remain enabled — a stricter probe that \
         disabled working rows would be a worse defect than the one being fixed"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-9 [acceptance] INV-2 + TEST-10 [acceptance] INV-4 — the defect-2
// reproduction.
//
// `https://huggingface.co/<nonexistent-org>` read `healthy` because the probe
// fetched the FIXED `huggingface.co/api/models` catalogue and never the row.
// The row is now probed with an `author=<org>` filter, so a nonexistent org
// produces an empty listing → `unverified`.
//
// INV-4 is the other half and is asserted in the same test: `unverified` must
// NOT auto-disable. `enabled` is what blocks downloads, so a stricter probe
// that disabled working repositories would trade a bad badge for an outage.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_nonexistent_hugging_face_org_is_unverified_and_stays_enabled() {
    let (registry_base, _registry_guard) = author_aware_registry_fixture().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LLM_REPOSITORY_HF_API_BASE".to_string(), registry_base)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    // ── The rejection: an org that does not exist. Reported live as the
    //    `https://huggingface.co/unused-model-path-v99` case.
    let missing_id = create_enabled_repo(
        &server,
        &admin.token,
        "hf-nonexistent-org",
        &format!("https://huggingface.co/org-{}", Uuid::new_v4()),
    )
    .await;
    let missing = probe_by_id(&server, &admin.token, missing_id).await;
    assert_eq!(
        missing["status"], "unverified",
        "INV-2: `healthy` must be positive evidence about THIS repository — a \
         row naming an org that lists no models cannot be graded on the hub's \
         global catalogue: {missing}"
    );
    assert_eq!(missing["success"], false, "{missing}");

    let (missing_enabled, missing_status, missing_reason) = read_health(&pool, missing_id).await;
    assert_eq!(missing_status, "unverified");
    assert_ne!(
        missing_status, "unhealthy",
        "INV-4: an unconfirmable row must not be recorded unhealthy — that is \
         what auto-disables it"
    );
    assert!(
        missing_enabled,
        "INV-4: the stricter probe must NOT auto-disable the row; `enabled` is \
         what blocks downloads, so this is the difference between a bad badge \
         and an outage"
    );
    assert!(
        missing_reason.is_some_and(|r| r.contains("org-")),
        "the reason must name the org that could not be confirmed, or an \
         operator cannot act on it"
    );

    // ── The happy-path counterpart, in the SAME test: an org-scoped row whose
    //    org DOES list models. `huggingface.co/<org>` is a LEGITIMATE base (the
    //    download path builds `<org>/<model>` from it), and a previous attempt
    //    at this fix was reverted for reporting every such row unverified.
    let good_id = create_enabled_repo(
        &server,
        &admin.token,
        "hf-real-org",
        &format!("https://huggingface.co/{KNOWN_ORG}"),
    )
    .await;
    let good = probe_by_id(&server, &admin.token, good_id).await;
    assert_eq!(
        good["status"], "healthy",
        "an org-scoped row naming a REAL org must stay healthy — this is the \
         case the earlier origin-only guard broke: {good}"
    );
    let (good_enabled, good_status, _) = read_health(&pool, good_id).await;
    assert_eq!(good_status, "healthy");
    assert!(good_enabled, "the working row stays enabled");

    // ── Third leg: a BARE Hugging Face origin still probes the catalogue and
    //    is still healthy, so the org filter did not become mandatory.
    let origin_id = create_enabled_repo(
        &server,
        &admin.token,
        "hf-bare-origin",
        "https://huggingface.co/",
    )
    .await;
    let origin = probe_by_id(&server, &admin.token, origin_id).await;
    assert_eq!(
        origin["status"], "healthy",
        "a bare Hugging Face origin has no org to filter on and keeps the \
         catalogue probe: {origin}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-11 [acceptance] INV-3 — an `Unknown`-kind row's PATH is part of its
// identity. `https://hf.co/models` is the reported case: the probe collapsed
// the row to its ORIGIN, so the `/models` path was never fetched.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_unknown_kind_rows_path_is_probed_not_discarded() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = pool_for(&server).await;

    // A self-hosted mirror serving a real listing at its ROOT `/api/models`,
    // and nothing under `/models/api/models`.
    let (base, _guard) = model_registry_fixture().await;

    // ── The rejection: the row carries a path the mirror does not serve. The
    //    probe must fetch `{base}/models/api/models`, not `{base}/api/models`.
    let pathed_id = create_enabled_repo(
        &server,
        &admin.token,
        "mirror-with-path",
        &format!("{base}/models"),
    )
    .await;
    let pathed = probe_by_id(&server, &admin.token, pathed_id).await;
    assert_eq!(
        pathed["status"], "unverified",
        "the row's own path must be probed — collapsing it to the origin grades \
         a URL the download path would never use: {pathed}"
    );
    let (pathed_enabled, _, _) = read_health(&pool, pathed_id).await;
    assert!(
        pathed_enabled,
        "INV-3/INV-4: unverified never auto-disables a self-hosted deployment"
    );

    // ── The happy-path counterpart: the SAME fixture as a bare origin is
    //    healthy, unchanged from today. Without this leg the assertion above
    //    would also pass if the Unknown branch had simply stopped working.
    let origin_id = create_enabled_repo(&server, &admin.token, "mirror-origin", &base).await;
    let origin = probe_by_id(&server, &admin.token, origin_id).await;
    assert_eq!(
        origin["status"], "healthy",
        "a bare-origin self-hosted mirror is unaffected by the path change: {origin}"
    );
    let (origin_enabled, origin_status, _) = read_health(&pool, origin_id).await;
    assert_eq!(origin_status, "healthy");
    assert!(origin_enabled);
}
