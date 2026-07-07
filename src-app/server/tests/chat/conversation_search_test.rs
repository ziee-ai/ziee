//! Conversation content-search integration tests (ITEM-4 / TEST-1, TEST-2).
//!
//! The list endpoint's `search` param matches a conversation's title OR the
//! text of any of its messages, and the paginated `total` reflects the filtered
//! set.

use reqwest::StatusCode;

use super::helpers;

async fn list_with_search(
    server: &crate::common::TestServer,
    token: &str,
    search: &str,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .query(&[("search", search)])
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "list should be 200");
    response.json().await.unwrap()
}

fn titles(body: &serde_json::Value) -> Vec<String> {
    body["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap_or("").to_string())
        .collect()
}

/// TEST-1: a term present only in a conversation's MESSAGE TEXT (not its title)
/// still matches; a conversation matching neither title nor content is excluded.
#[tokio::test]
async fn test_search_matches_message_content_not_just_title() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // A: title has NO match; its message text does.
    let conv_a =
        helpers::create_conversation(&server, &user.token, None, Some("Grocery plans")).await;
    let branch_a = helpers::parse_uuid(&conv_a["active_branch_id"]);
    helpers::seed_text_message(
        &server.database_url,
        branch_a,
        "user",
        "I want a pineapple upside-down cake recipe",
    )
    .await;

    // C: neither title nor content matches.
    let conv_c =
        helpers::create_conversation(&server, &user.token, None, Some("Weather notes")).await;
    let branch_c = helpers::parse_uuid(&conv_c["active_branch_id"]);
    helpers::seed_text_message(&server.database_url, branch_c, "user", "it is sunny today").await;

    let body = list_with_search(&server, &user.token, "pineapple").await;
    let found = titles(&body);

    assert!(
        found.contains(&"Grocery plans".to_string()),
        "content match should be returned, got {found:?}",
    );
    assert!(
        !found.contains(&"Weather notes".to_string()),
        "non-matching conversation must be excluded, got {found:?}",
    );
    assert_eq!(body["total"].as_i64().unwrap(), 1, "total = filtered count");
}

/// TEST-2: search matches title OR content (union), and `total` equals the
/// FILTERED count, so pagination stays consistent.
#[tokio::test]
async fn test_search_title_or_content_and_filtered_total() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // A: content match only.
    let conv_a =
        helpers::create_conversation(&server, &user.token, None, Some("Dinner ideas")).await;
    let branch_a = helpers::parse_uuid(&conv_a["active_branch_id"]);
    helpers::seed_text_message(&server.database_url, branch_a, "user", "how about salmon tonight")
        .await;

    // B: title match only (case-insensitive), no seeded content.
    helpers::create_conversation(&server, &user.token, None, Some("SALMON fishing trip")).await;

    // C: unrelated.
    let conv_c =
        helpers::create_conversation(&server, &user.token, None, Some("Book club")).await;
    let branch_c = helpers::parse_uuid(&conv_c["active_branch_id"]);
    helpers::seed_text_message(&server.database_url, branch_c, "user", "we read a novel").await;

    let body = list_with_search(&server, &user.token, "salmon").await;
    let found = titles(&body);

    assert!(found.contains(&"Dinner ideas".to_string()), "content match, got {found:?}");
    assert!(found.contains(&"SALMON fishing trip".to_string()), "title match, got {found:?}");
    assert!(!found.contains(&"Book club".to_string()), "unrelated excluded, got {found:?}");
    assert_eq!(
        body["total"].as_i64().unwrap(),
        2,
        "total must be the filtered count (2), not the unfiltered 3",
    );

    // A term matching nothing yields an empty page + zero total.
    let empty = list_with_search(&server, &user.token, "zzq-nonexistent").await;
    assert_eq!(empty["conversations"].as_array().unwrap().len(), 0);
    assert_eq!(empty["total"].as_i64().unwrap(), 0);
}

/// LIKE metacharacters in the search term match LITERALLY, not as wildcards
/// (regression for the escape fix): `user_data` must not match `userXdata`.
#[tokio::test]
async fn test_search_escapes_like_metacharacters() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    helpers::create_conversation(&server, &user.token, None, Some("user_data notes")).await;
    helpers::create_conversation(&server, &user.token, None, Some("userXdata notes")).await;

    // If `_` were treated as a wildcard, this would match BOTH; escaped, it
    // matches only the literal-underscore title.
    let body = list_with_search(&server, &user.token, "user_data").await;
    let found = titles(&body);
    assert_eq!(found, vec!["user_data notes".to_string()], "got {found:?}");
    assert_eq!(body["total"].as_i64().unwrap(), 1);
}
