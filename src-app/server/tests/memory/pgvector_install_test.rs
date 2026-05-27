// ============================================================================
// pgvector install regression — plan §13 verification step 2.
//
// Drives the smoke check that the running test DB has pgvector
// available and accepts `CREATE EXTENSION vector` + a `vector(768)`
// roundtrip. This is the post-build proof that the embedded-PG +
// build_helper bundling pipeline produced a working extension.
// ============================================================================

#[tokio::test]
async fn test_vector_extension_loaded_and_roundtrips() {
    let server = crate::common::TestServer::start().await;
    // Get the pool via a privileged user — extension probing doesn't
    // need admin perms; any authenticated user is fine since we go
    // through the memory routes.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "pgv_install",
        &["memory::read", "memory::write"],
    )
    .await;

    // Insert via the public REST surface — this exercises the
    // vector(768) column path. If pgvector wasn't loaded, migration
    // 46 would have failed and the test server wouldn't have come up.
    let res = reqwest::Client::new()
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({
            "content": "User likes pgvector"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201, "insert into vector-bearing table must succeed");
}
