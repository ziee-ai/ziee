//! SSRF confinement of the credential-carrying repository probe — TEST-4,
//! TEST-5, TEST-6, TEST-12. Pins INV-1.
//!
//! Realizes `docs/design/llm-repository-probe-integrity.md` §1.
//!
//! The defect: `test_repository_connectivity` built its client with
//! `OutboundUrlPolicy::PUBLIC_HTTP_OR_HTTPS` but never called
//! `validate_outbound_url` on the URLs it fetched. Those are not equivalent —
//! `build_validated_client` installs a `GuardingResolver` that filters
//! **DNS-resolved** addresses, and reqwest never consults a DNS resolver for an
//! **IP-literal** host. So an IP literal reached the network unchecked, with the
//! repository's credentials attached by `apply_auth`.
//!
//! `POST /api/llm-repositories/test` additionally never validated
//! `auth_test_api_endpoint` at all, so the cloud metadata endpoint
//! (`169.254.169.254`) was reachable — in a release build — carrying a bearer
//! token.
//!
//! Every rejection assertion below ships with its happy-path counterpart in the
//! SAME test, so a rejection that "passes" because the probe broke some other
//! way cannot go unnoticed.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use axum::{Router, extract::State, http::HeaderMap, http::StatusCode, routing::get};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const REPO_ADMIN_PERMS: &[&str] = &[
    "llm_repositories::read",
    "llm_repositories::create",
    "llm_repositories::edit",
    "llm_repositories::delete",
];

/// The AWS/GCP/Azure link-local metadata address. Blocked by EVERY policy in
/// this module — `DEV_LOCAL` (debug) and `PUBLIC_HTTP_OR_HTTPS` (release) both
/// set `allow_link_local: false` — so a test asserting it is refused proves the
/// same thing in both build profiles. Nothing listens here; that is the point.
const IMDS_ENDPOINT: &str = "http://169.254.169.254/latest/meta-data/iam/security-credentials/";

/// An RFC1918 address, likewise blocked under both profiles.
const RFC1918_ENDPOINT: &str = "http://10.0.0.5/whoami";

/// Aborts the fixture's serving task on scope exit — success OR panic-unwind.
struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// What a fixture observed: one entry per received request, holding that
/// request's `Authorization` header if it carried one.
type Observed = Arc<Mutex<Vec<Option<String>>>>;

/// A loopback fixture that RECORDS every request it receives (and the
/// credential it was handed) and answers 200. This is what makes the leak
/// concrete: the assertion is not "the probe returned an error", it is "the
/// destination never saw the token".
async fn recording_fixture() -> (String, Observed, AbortOnDrop) {
    let observed: Observed = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new()
        .fallback(get(
            |State(seen): State<Observed>, headers: HeaderMap| async move {
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                seen.lock().expect("observed lock").push(auth);
                (StatusCode::OK, "ok")
            },
        ))
        .with_state(observed.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let guard = AbortOnDrop(tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    }));
    (format!("http://127.0.0.1:{}", addr.port()), observed, guard)
}

fn hits(observed: &Observed) -> Vec<Option<String>> {
    observed.lock().expect("observed lock").clone()
}

/// `POST /llm-repositories/test` — the UN-SAVED probe path.
async fn test_unsaved(
    server: &TestServer,
    token: &str,
    url: &str,
    test_endpoint: &str,
    bearer: &str,
) -> (StatusCode, Value) {
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": "probe-target",
            "url": url,
            "auth_type": "bearer_token",
            "auth_config": {
                "token": bearer,
                "auth_test_api_endpoint": test_endpoint,
            },
        }))
        .send()
        .await
        .expect("probe request failed");
    let status = res.status();
    (status, res.json().await.expect("probe response is JSON"))
}

/// Does this probe message describe a POLICY refusal rather than a network
/// failure? Before the fix the IMDS leg produced a transport error (a 10s
/// connect timeout); after it, the destination is refused before any socket is
/// opened. Distinguishing the two is the whole point of the test.
fn is_policy_refusal(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    (m.contains("auth_test_api_endpoint") || m.contains("not permitted") || m.contains("blocked"))
        && !m.contains("timed out")
        && !m.contains("connection failed")
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-4 [acceptance] INV-1 — the defect-1 reproduction, on the un-saved path.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn imds_probe_endpoint_is_refused_and_never_receives_the_credential() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;

    // ── The rejection ────────────────────────────────────────────────────
    // An admin points the probe's credential endpoint at the cloud metadata
    // service. `apply_auth` would attach the bearer token to it.
    let (status, body) = test_unsaved(
        &server,
        &admin.token,
        "https://huggingface.co/",
        IMDS_ENDPOINT,
        "super-secret-token",
    )
    .await;

    assert_eq!(
        status, 200,
        "handler answers 200 with a failure body (DEC-3)"
    );
    assert_eq!(
        body["success"], false,
        "a probe aimed at IMDS must not succeed: {body}"
    );
    assert_eq!(
        body["status"], "unhealthy",
        "a forbidden configured destination is actionable, not `unverified` (DEC-2): {body}"
    );
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        is_policy_refusal(message),
        "the endpoint must be REFUSED BY POLICY before any socket is opened — a \
         connect timeout would mean the request was actually attempted against \
         169.254.169.254 with the token attached. message={message:?}"
    );

    // Same refusal for RFC1918.
    let (_, rfc1918) = test_unsaved(
        &server,
        &admin.token,
        "https://huggingface.co/",
        RFC1918_ENDPOINT,
        "super-secret-token",
    )
    .await;
    assert_eq!(
        rfc1918["success"], false,
        "RFC1918 must be refused: {rfc1918}"
    );
    assert!(
        is_policy_refusal(rfc1918["message"].as_str().unwrap_or_default()),
        "RFC1918 must be a policy refusal, not a transport error: {rfc1918}"
    );

    // ── The happy-path counterpart, in the SAME test ─────────────────────
    // A PERMITTED endpoint is still fetched, and still carries the credential.
    // Without this leg, a probe that simply stopped working would pass the
    // assertions above.
    let (fixture_url, observed, _guard) = recording_fixture().await;
    let (_, ok_body) = test_unsaved(
        &server,
        &admin.token,
        "https://huggingface.co/",
        &format!("{fixture_url}/whoami"),
        "super-secret-token",
    )
    .await;

    let seen = hits(&observed);
    assert_eq!(
        seen.len(),
        1,
        "the permitted endpoint MUST still be probed exactly once — otherwise \
         the rejection above proves nothing. body={ok_body}"
    );
    assert_eq!(
        seen[0].as_deref(),
        Some("Bearer super-secret-token"),
        "the probe attaches the repository credential — this is exactly what \
         the IMDS leg must never be allowed to deliver"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-5 — the same confinement on the SAVED-row path, plus the recorded
// health outcome.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn saved_row_probe_refuses_a_forbidden_endpoint_without_contacting_it() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test pool");

    // A row that is legitimate at creation time.
    let (fixture_url, observed, _guard) = recording_fixture().await;
    let create = reqwest::Client::new()
        .post(server.api_url("/llm-repositories"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "saved-probe-target",
            "url": "https://huggingface.co/",
            "auth_type": "bearer_token",
            "enabled": true,
            "auth_config": {
                "token": "persisted-secret",
                "auth_test_api_endpoint": format!("{fixture_url}/whoami"),
            },
        }))
        .send()
        .await
        .expect("create request failed");
    let create_status = create.status();
    let created: Value = create.json().await.expect("create response is JSON");
    assert_eq!(
        create_status, 201,
        "create should return 201; body={created}"
    );
    let repo_id = Uuid::parse_str(created["id"].as_str().expect("id")).expect("uuid");

    let baseline = hits(&observed).len();

    // ── The rejection: steer the probe at IMDS via a form override ───────
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "auth_config": { "auth_test_api_endpoint": IMDS_ENDPOINT },
        }))
        .send()
        .await
        .expect("probe request failed");
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.expect("probe response is JSON");
    assert_eq!(
        body["success"], false,
        "IMDS override must be refused: {body}"
    );
    assert!(
        is_policy_refusal(body["message"].as_str().unwrap_or_default()),
        "must be a policy refusal, not a transport error: {body}"
    );
    assert_eq!(
        hits(&observed).len(),
        baseline,
        "the refused probe must not have contacted the original fixture either"
    );

    // ── The happy-path counterpart: the unmodified row still probes ──────
    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/llm-repositories/{repo_id}/test")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({}))
        .send()
        .await
        .expect("probe request failed");
    assert_eq!(res.status(), 200);
    let ok_body: Value = res.json().await.expect("probe response is JSON");
    let after = hits(&observed);
    assert!(
        after.len() > baseline,
        "the row's own permitted endpoint must still be probed; body={ok_body}"
    );
    assert_eq!(
        after.last().unwrap().as_deref(),
        Some("Bearer persisted-secret"),
        "the persisted credential still flows to the row's OWN endpoint"
    );

    // The row survived: a refused probe must not have silently corrupted state
    // beyond the recorded health outcome.
    let row = sqlx::query!(
        r#"SELECT enabled, last_health_check_status FROM llm_repositories WHERE id = $1"#,
        repo_id,
    )
    .fetch_one(&pool)
    .await
    .expect("read repo row");
    assert!(
        !row.last_health_check_status.is_empty(),
        "the probe outcome is recorded"
    );
    let _ = row.enabled;
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-6 — the rejected-endpoint response SHAPE (DEC-3): the module's existing
// 200 + success:false contract, never a 500 or a panic.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rejected_endpoint_uses_the_modules_existing_failure_shape() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;

    for endpoint in [IMDS_ENDPOINT, RFC1918_ENDPOINT, "file:///etc/passwd"] {
        let (status, body) = test_unsaved(
            &server,
            &admin.token,
            "https://huggingface.co/",
            endpoint,
            "tok",
        )
        .await;
        assert_eq!(
            status, 200,
            "endpoint {endpoint}: expected 200; body={body}"
        );
        assert_eq!(body["success"], false, "endpoint {endpoint}: {body}");
        assert!(
            body["message"].is_string() && body["status"].is_string(),
            "endpoint {endpoint}: response keeps the documented shape; body={body}"
        );
    }

    // Happy-path counterpart: a permitted endpoint produces the same SHAPE with
    // the opposite outcome, so the assertions above are not just "any 200".
    let (fixture_url, _observed, _guard) = recording_fixture().await;
    let (status, body) = test_unsaved(
        &server,
        &admin.token,
        "https://huggingface.co/",
        &format!("{fixture_url}/whoami"),
        "tok",
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        body["message"].is_string() && body["status"].is_string(),
        "permitted endpoint keeps the same response shape; body={body}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// TEST-12 — the endpoint ITEM-4 edits is permission-gated. 401 / 403 / 200.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn unsaved_probe_endpoint_is_permission_gated() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", REPO_ADMIN_PERMS).await;
    let outsider = create_user_with_permissions(&server, "outsider", &[]).await;
    let (fixture_url, _observed, _guard) = recording_fixture().await;
    let payload = json!({
        "name": "probe-target",
        "url": "https://huggingface.co/",
        "auth_type": "bearer_token",
        "auth_config": {
            "token": "tok",
            "auth_test_api_endpoint": format!("{fixture_url}/whoami"),
        },
    });

    // Unauthenticated → 401.
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .json(&payload)
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), 401, "unauthenticated must be refused");

    // Authenticated but lacking `llm_repositories::read` → 403.
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .json(&payload)
        .send()
        .await
        .expect("request failed");
    assert_eq!(
        res.status(),
        403,
        "a user without llm_repositories::read must be refused"
    );

    // Positive control: the SAME payload from a permitted user reaches the
    // handler. Without this, the two refusals above could both be a broken route.
    let res = reqwest::Client::new()
        .post(server.api_url("/llm-repositories/test"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("request failed");
    assert_eq!(
        res.status(),
        200,
        "a permitted user still reaches the probe handler"
    );
}
