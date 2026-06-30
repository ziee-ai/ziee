//! Full desktop-backend lifecycle test.
//!
//! Exercises the real production path end to end: the shared `TestServer`
//! harness spawns the actual `ziee-desktop --headless` binary against an
//! isolated database, runs migrations, and waits for the HTTP listener to
//! bind. We then confirm the live unauthenticated health endpoint responds,
//! and dropping the `TestServer` tears the process down (clean shutdown).
//!
//! Runs headless (no Tauri window), so unlike `spawn_binary_smoke` (which
//! boots the GUI release binary and needs a display session) it runs in the
//! default suite. Replaces the old `backend_tests::test_full_server_lifecycle`
//! stub, which was `#[ignore]`'d and asserted nothing real.

#[tokio::test]
async fn desktop_backend_full_lifecycle() {
    // Boot: spawns ziee-desktop --headless, migrates, waits for readiness.
    let server = crate::common::TestServer::start_desktop().await;

    // Serve: the unauthenticated health endpoint must respond (desktop CORS
    // is permissive, so a plain GET works). This proves the full chain —
    // real binary → migrations → bound listener → router — is live.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client");
    let resp = client
        .get(server.api_url("/health"))
        .send()
        .await
        .expect("GET /api/health");
    assert!(
        resp.status().is_success(),
        "GET /api/health should succeed, got {}",
        resp.status()
    );

    // Shutdown: dropping `server` kills + reaps the child process.
    drop(server);
}
