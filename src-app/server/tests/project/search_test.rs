//! Integration tests for the `GET /projects?search=` name/description filter
//! (feature: project-search). Mirrors the crud_test harness usage.

use reqwest::StatusCode;
use serde_json::json;

use super::helpers;
use crate::common::TestServer;
use crate::common::test_helpers::{TestUser, create_user_with_permissions};

/// GET /projects with an optional `search` term; returns the parsed body.
async fn list(server: &TestServer, user: &TestUser, search: Option<&str>) -> serde_json::Value {
    let path = match search {
        Some(q) => format!("/projects?search={}", urlencoding(q)),
        None => "/projects".to_string(),
    };
    let resp = reqwest::Client::new()
        .get(server.api_url(&path))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "list {path}");
    resp.json().await.unwrap()
}

/// Minimal percent-encoding for the few chars our test terms use. Encode `%`
/// first (→ %25) so a literal-percent term round-trips to the server, then space.
fn urlencoding(s: &str) -> String {
    s.replace('%', "%25").replace(' ', "%20")
}

fn names(body: &serde_json::Value) -> Vec<String> {
    body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

/// TEST-3 — substring match on name, case-insensitive; total reflects filter.
#[tokio::test]
async fn search_by_name_case_insensitive() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "user", helpers::full_project_permissions()).await;

    helpers::create_project(&server, &user, "Roadmap").await;
    helpers::create_project(&server, &user, "Backlog").await;
    helpers::create_project(&server, &user, "Design").await;

    let body = list(&server, &user, Some("road")).await;
    assert_eq!(body["total"], 1, "only Roadmap matches 'road'");
    assert_eq!(names(&body), vec!["Roadmap".to_string()]);

    // Case-insensitive.
    let upper = list(&server, &user, Some("ROAD")).await;
    assert_eq!(upper["total"], 1, "ILIKE is case-insensitive");
    assert_eq!(names(&upper), vec!["Roadmap".to_string()]);

    // A term matching nothing yields an empty page + zero total.
    let none = list(&server, &user, Some("zzz")).await;
    assert_eq!(none["total"], 0);
    assert!(none["projects"].as_array().unwrap().is_empty());
}

/// TEST-4 — substring match also covers the description column.
#[tokio::test]
async fn search_matches_description() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "user", helpers::full_project_permissions()).await;

    helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Alpha", "description": "the quarterly roadmap doc" }),
    )
    .await;
    helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Beta", "description": "meeting notes" }),
    )
    .await;

    let body = list(&server, &user, Some("roadmap")).await;
    assert_eq!(body["total"], 1, "matched via description");
    assert_eq!(names(&body), vec!["Alpha".to_string()]);
}

/// TEST-5 — blank and absent search both return ALL projects (no filter).
#[tokio::test]
async fn blank_and_absent_return_all() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "user", helpers::full_project_permissions()).await;

    helpers::create_project(&server, &user, "One").await;
    helpers::create_project(&server, &user, "Two").await;
    helpers::create_project(&server, &user, "Three").await;

    let absent = list(&server, &user, None).await;
    assert_eq!(absent["total"], 3, "no search param → all");

    let blank = list(&server, &user, Some("")).await;
    assert_eq!(blank["total"], 3, "blank search normalizes to no filter");

    let whitespace = list(&server, &user, Some("   ")).await;
    assert_eq!(whitespace["total"], 3, "whitespace-only search → no filter");
}

/// TEST-6 — search never widens the per-user ownership scope.
#[tokio::test]
async fn search_is_ownership_scoped() {
    let server = TestServer::start().await;
    let user_a = create_user_with_permissions(&server, "alice", helpers::full_project_permissions()).await;
    let user_b = create_user_with_permissions(&server, "bob", helpers::full_project_permissions()).await;

    // Bob owns a "Roadmap"; Alice owns nothing matching.
    helpers::create_project(&server, &user_b, "Roadmap").await;
    helpers::create_project(&server, &user_a, "Backlog").await;

    let body = list(&server, &user_a, Some("road")).await;
    assert_eq!(body["total"], 0, "Alice must not see Bob's matching project");
    assert!(body["projects"].as_array().unwrap().is_empty());
}

/// GET /projects with explicit search + limit (raw, for pagination assertions).
async fn list_paged(
    server: &TestServer,
    user: &TestUser,
    search: &str,
    limit: i64,
) -> serde_json::Value {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects?search={search}&limit={limit}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.unwrap()
}

/// TEST-8 — the filtered `total` (COUNT) reflects the FULL matching set even
/// when the page is truncated by `limit` (COUNT ignores LIMIT). This is the
/// pagination-consistency case the first test round missed.
#[tokio::test]
async fn filtered_total_survives_page_truncation() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "user", helpers::full_project_permissions()).await;

    helpers::create_project(&server, &user, "Report Q1").await;
    helpers::create_project(&server, &user, "Report Q2").await;
    helpers::create_project(&server, &user, "Report Q3").await;
    helpers::create_project(&server, &user, "Unrelated").await;

    let body = list_paged(&server, &user, "report", 2).await;
    assert_eq!(body["total"], 3, "COUNT reflects all 3 matches, ignoring LIMIT");
    assert_eq!(
        body["projects"].as_array().unwrap().len(),
        2,
        "page is truncated to limit=2"
    );
}

/// TEST-9 — a term matching MULTIPLE projects returns all of them; and LIKE
/// metacharacters in the term are treated as wildcards (not escaped), matching
/// the mcp convention (DEC-7) — documented here rather than left implicit.
#[tokio::test]
async fn multi_match_and_wildcard_metacharacters() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "user", helpers::full_project_permissions()).await;

    helpers::create_project(&server, &user, "Repair log").await;
    helpers::create_project(&server, &user, "Report doc").await;
    helpers::create_project(&server, &user, "Design notes").await;

    // "re" is a substring of both "Repair" and "Report" (case-insensitive).
    let multi = list(&server, &user, Some("re")).await;
    assert_eq!(multi["total"], 2, "'re' matches Repair + Report");
    let mut got = names(&multi);
    got.sort();
    assert_eq!(got, vec!["Repair log".to_string(), "Report doc".to_string()]);

    // A bare '%' term is an unescaped ILIKE wildcard → matches everything the
    // user owns (still scoped to their own rows). Encodes the DEC-7 behavior.
    let wildcard = list(&server, &user, Some("%")).await;
    assert_eq!(wildcard["total"], 3, "'%' is a wildcard, matches all own projects");
}
