use crate::common::test_helpers;
use crate::common::TestServer;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// Deterministic id of the built-in code_sandbox server (the transient-file producer).
/// Recomputed inline rather than imported (the server-crate fn isn't re-exported).
fn code_sandbox_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"code-sandbox.ziee.internal")
}

fn ziee_link(uri: &str, name: &str) -> ziee::ResourceLink {
    ziee::ResourceLink {
        uri: uri.to_string(),
        name: Some(name.to_string()),
        mime_type: Some("text/plain".to_string()),
        size: None,
        is_saved: None,
        file_id: None,
        version_id: None,
        version: None,
    }
}

async fn user_id(server: &TestServer) -> Uuid {
    let user = test_helpers::create_user_with_permissions(server, "rluser", &[]).await;
    Uuid::parse_str(&user.user_id).expect("user_id is a uuid")
}

/// The single test that drives the full INGEST path (and therefore initializes the
/// in-process globals). One `persist_links` call over a MIXED link set proves, in one shot:
///  - `is_saved:true`           → referenced, never re-saved, URI untouched
///  - `ziee://` under the root  → ingested (byte round-trip), URI rewritten to
///                                `/api/files/{id}`, DB row `created_by="workflow"`
///  - `ziee://` OUTSIDE the root → rejected, not saved, URI blanked (guard #3)
///
/// `workflow_run_id` is `None` here (the chat path): run-linking is no longer a no-op —
/// passing a non-existent run id would FK-violate and orphan-delete the ingested file.
/// The real run-link branch is exercised by
/// `persist_links_run_link_attributes_ingested_file_to_real_run`.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn persist_ingests_ziee_under_root_and_handles_mixed_links() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    // Point the process-global Repos + file store at THIS test's DB + a temp dir.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    // ALWAYS (re-)point the global Repos at THIS test's DB. `init_repositories`
    // swaps the active pool on every call; gating it behind `is_repos_initialized`
    // would leave Repos bound to whichever earlier serial test initialized first
    // (whose per-test DB is already torn down) → cross-DB FK failures here.
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_store_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);

    // A host workspace "root" with a real artifact under it; plus an unrelated file outside.
    let root = std::env::temp_dir().join(format!("ziee_rl_ws_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("out.csv");
    let payload = b"a,b,c\n1,2,3\n";
    std::fs::write(&artifact, payload).unwrap();
    let outside = std::env::temp_dir().join(format!("ziee_rl_outside_{}.txt", Uuid::new_v4()));
    std::fs::write(&outside, b"nope").unwrap();

    let mut saved_link = ziee_link(
        "https://h/api/files/abc/download-with-token?token=xyz",
        "ref.pdf",
    );
    saved_link.is_saved = Some(true);
    let mut links = vec![
        saved_link,
        ziee_link(&format!("ziee://{}", artifact.display()), "out.csv"),
        ziee_link(&format!("ziee://{}", outside.display()), "escape.txt"),
    ];

    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        None, // chat path: no run-link (run-linking is exercised by the real-run test)
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[root.clone()],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    // is_saved:true → referenced, not re-saved, URI untouched.
    assert_eq!(outcome.referenced.len(), 1, "one referenced (is_saved) link");
    assert_eq!(outcome.referenced[0].0, "ref.pdf");
    assert!(
        links[0].uri.contains("download-with-token"),
        "is_saved URI must be untouched"
    );
    assert!(links[0].file_id.is_none());

    // ziee:// under root → exactly one ingested, stamped onto the correct link index.
    assert_eq!(outcome.saved.len(), 1, "exactly one ingested");
    let art = &outcome.saved[0];
    assert_eq!(art.link_idx, 1, "stamped onto the correct link");
    assert_eq!(art.filename, "out.csv");
    assert_eq!(
        links[1].uri,
        format!("/api/files/{}", art.file_id),
        "guard #3: ziee:// rewritten to /api/files/{{id}}"
    );
    assert_eq!(links[1].file_id, Some(art.file_id));
    assert!(!links[1].uri.contains("ziee://"));

    // ziee:// outside root → rejected, not saved, URI blanked (guard #3 — no host-path leak).
    assert!(
        links[2].uri.is_empty(),
        "rejected ziee:// URI must be blanked, got {:?}",
        links[2].uri
    );
    assert!(links[2].file_id.is_none());

    // Byte round-trip: the saved original blob equals the source bytes.
    let stored = ziee::get_file_storage()
        .load_original(uid, art.file_id, "csv")
        .await
        .expect("blob readable");
    assert_eq!(stored, payload, "saved blob must equal the source bytes");

    // DB row provenance.
    let row = sqlx::query!(
        "SELECT created_by, filename FROM files WHERE id = $1",
        art.file_id
    )
    .fetch_one(&pool)
    .await
    .expect("file row created");
    assert_eq!(row.created_by, "workflow");
    assert_eq!(row.filename, "out.csv");

    pool.close().await;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&store_dir).ok();
    std::fs::remove_file(&outside).ok();
}

/// Guard #1: a `ziee://` link from a NON-built-in (external/user) server is ignored — not
/// ingested, and its raw URI is blanked (guard #3). Short-circuits before the save tail.
#[tokio::test]
async fn ziee_link_from_external_server_is_ignored() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let root = std::env::temp_dir().join(format!("ziee_rl_ext_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("secret.txt");
    std::fs::write(&artifact, b"top secret").unwrap();

    let mut links = vec![ziee_link(&format!("ziee://{}", artifact.display()), "secret.txt")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        None,
        Uuid::new_v4(), // external/user server id → not a trusted emitter
        false,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[root.clone()],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    assert!(outcome.saved.is_empty(), "external ziee:// must not be ingested");
    assert!(
        !links[0].uri.starts_with("ziee://"),
        "untrusted ziee:// URI must be blanked (no host-path leak)"
    );
    assert!(links[0].file_id.is_none());

    std::fs::remove_dir_all(&root).ok();
}

/// A `ziee://workflow-runs/...` handle (workflow_mcp's resource dialect — a relative
/// remainder, NOT a host path) is never ingested as a host file by persist_links, and is
/// left INTACT: only absolute-host-path `ziee://` links are blanked, so workflow handles in
/// `structured_content` / resource_links survive. Short-circuits before the save tail.
#[tokio::test]
async fn ziee_workflow_runs_handle_is_not_ingested_and_preserved() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let handle = "ziee://workflow-runs/abc/outputs/x.json";
    let mut links = vec![ziee_link(handle, "x.json")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        None,
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    assert!(
        outcome.saved.is_empty(),
        "a workflow-runs handle must not be ingested as a host file"
    );
    assert_eq!(
        links[0].uri, handle,
        "a non-host-path ziee:// (workflow handle) is left intact — it carries no host-path disclosure"
    );
}

/// A `ziee://` under the allowed root that passes confinement but FAILS to read (here it
/// points at a directory, so `fs::read` errors) is not ingested and its URI is blanked
/// (guard #3 — the round-1 save-failure gap). `fs::read` fails before the save tail, so no
/// in-process globals are needed.
#[tokio::test]
async fn ziee_link_read_failure_is_blanked() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let root = std::env::temp_dir().join(format!("ziee_rl_readfail_{}", Uuid::new_v4()));
    let subdir = root.join("a_directory");
    std::fs::create_dir_all(&subdir).unwrap(); // a directory under the root → read() fails

    let mut links = vec![ziee_link(&format!("ziee://{}", subdir.display()), "dir")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        None,
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[root.clone()],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    assert!(outcome.saved.is_empty(), "unreadable ziee:// must not be ingested");
    assert!(
        links[0].uri.is_empty(),
        "save-failure ziee:// URI must be blanked, got {:?}",
        links[0].uri
    );

    std::fs::remove_dir_all(&root).ok();
}

/// The HTTP/loopback branch with no `jwt_secret` (the workflow-dispatcher context) skips the
/// fetch entirely — no save, and a non-`ziee://` URI is left untouched. Short-circuits
/// before the save tail.
#[tokio::test]
async fn http_link_without_jwt_secret_is_skipped() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let url = "http://127.0.0.1:9/some/artifact.bin".to_string();
    let mut links = vec![ziee_link(&url, "artifact.bin")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        None,
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[],
        None, // no jwt secret → HTTP branch is skipped (the dispatcher path)
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    assert!(
        outcome.saved.is_empty(),
        "HTTP link must be skipped without a jwt secret"
    );
    assert_eq!(links[0].uri, url, "a non-ziee:// URI is left untouched");
}

/// Run-link recall / attribution branch (PR #110, "C4"): when `persist_links` is called
/// with `workflow_run_id = Some(real_run)`, each newly-ingested resource_link file is linked
/// to its producing run via `Repos.file.set_workflow_run_id` AFTER the save loop — so a later
/// `tool_result` recall whose blocks carry that resource_link is attributable to (and A5
/// cascade-deletable with) the run that created it. The mixed-link test above passes a
/// *random* run id (a documented no-op that never reaches a real run row); this exercises the
/// SUCCESS path against a REAL `workflow_runs` row and asserts (a) the ingested file's
/// `workflow_run_id` is set (not orphan-deleted) and (b) the saved `/api/files/{id}` reference
/// still resolves to the original bytes — i.e. the recalled link is a live, run-attributed handle.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn persist_links_run_link_attributes_ingested_file_to_real_run() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    // Same shared DB the global Repos uses (first-call-wins init guard).
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    // ALWAYS (re-)point the global Repos at THIS test's DB. `init_repositories`
    // swaps the active pool on every call; gating it behind `is_repos_initialized`
    // would leave Repos bound to whichever earlier serial test initialized first
    // (whose per-test DB is already torn down) → cross-DB FK failures here.
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_store_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);

    // A REAL workflow + run row so `files.workflow_run_id`'s FK is satisfiable (a random id
    // would FK-violate and the file would be orphan-deleted instead of linked).
    let workflow_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO workflows \
            (id, name, extracted_path, bundle_sha256, bundle_size_bytes, file_count, entry_point, scope, owner_user_id) \
         VALUES ($1, 'rl-run-link-wf', '/tmp/rl-none', 'deadbeef', 0, 0, 'workflow.yaml', 'user', $2)",
        workflow_id,
        uid,
    )
    .execute(&pool)
    .await
    .expect("insert workflow");
    let run_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO workflow_runs (id, workflow_id, user_id) VALUES ($1, $2, $3)",
        run_id,
        workflow_id,
        uid,
    )
    .execute(&pool)
    .await
    .expect("insert workflow_run");

    // A host workspace root with a real run-produced artifact under it.
    let root = std::env::temp_dir().join(format!("ziee_rl_ws_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("chart.png");
    let payload = b"\x89PNG\r\n\x1a\n-fake-chart-bytes";
    std::fs::write(&artifact, payload).unwrap();

    let mut links = vec![ziee_link(&format!("ziee://{}", artifact.display()), "chart.png")];

    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "workflow",
        Some(run_id), // REAL run → the run-link branch must set files.workflow_run_id
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[root.clone()],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    // Exactly one ingested, rewritten to the model-facing recall handle (no host-path leak).
    assert_eq!(outcome.saved.len(), 1, "exactly one ingested");
    let art = &outcome.saved[0];
    assert_eq!(
        links[0].uri,
        format!("/api/files/{}", art.file_id),
        "ziee:// rewritten to the /api/files/{{id}} recall handle"
    );
    assert!(!links[0].uri.contains("ziee://"), "no host-path leak in the recalled URI");

    // Run-link branch: the ingested file is attributed to the producing run — it SURVIVES
    // (was not orphan-deleted) and carries the run id.
    let row = sqlx::query!(
        "SELECT workflow_run_id FROM files WHERE id = $1",
        art.file_id
    )
    .fetch_one(&pool)
    .await
    .expect("ingested file row survives (not orphaned)");
    assert_eq!(
        row.workflow_run_id,
        Some(run_id),
        "the ingested resource_link file must be linked to its producing run"
    );

    // The recalled reference is genuinely resolvable: its bytes round-trip from storage.
    let stored = ziee::get_file_storage()
        .load_original(uid, art.file_id, "png")
        .await
        .expect("saved /api/files/{id} blob is recallable");
    assert_eq!(stored, payload, "recalled resource_link resolves to the original bytes");

    pool.close().await;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&store_dir).ok();
}

/// Chat path (NOT a workflow run): a code_sandbox `ziee://` artifact saved
/// during a chat passes `workflow_run_id = None`, so the ingested file must be
/// created but NOT linked to any run (`files.workflow_run_id IS NULL`) — the
/// counterpart to the workflow-path test above, which links the file to a run.
/// This pins the code_sandbox → file-store integration for the chat dispatcher.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn code_sandbox_chat_path_persists_artifact_without_run_link() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    // ALWAYS (re-)point the global Repos at THIS test's DB. `init_repositories`
    // swaps the active pool on every call; gating it behind `is_repos_initialized`
    // would leave Repos bound to whichever earlier serial test initialized first
    // (whose per-test DB is already torn down) → cross-DB FK failures here.
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_chat_store_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);

    let root = std::env::temp_dir().join(format!("ziee_rl_chat_ws_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("plot.png");
    let payload = b"\x89PNG\r\n\x1a\nfake-chart-bytes";
    std::fs::write(&artifact, payload).unwrap();

    let mut links = vec![ziee_link(&format!("ziee://{}", artifact.display()), "plot.png")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "user",        // chat provenance, not "workflow"
        None,          // workflow_run_id = None → no run link (the chat path)
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[], // trusted_hosts: none — existing behavior for these cases
        &[root.clone()],
        None,
        None, // jwt_issuer
        None, // jwt_audience
    )
    .await
    .expect("persist_links");

    // The artifact is ingested + the ziee:// URI rewritten to /api/files/{id}.
    assert_eq!(outcome.saved.len(), 1, "code_sandbox artifact ingested");
    let art = &outcome.saved[0];
    assert_eq!(links[0].uri, format!("/api/files/{}", art.file_id));
    assert!(!links[0].uri.contains("ziee://"), "host path must not leak to the client");

    // It is NOT linked to any workflow run (chat path).
    let run_id: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM files WHERE id = $1")
            .bind(art.file_id)
            .fetch_one(&pool)
            .await
            .expect("file row exists");
    assert!(run_id.is_none(), "chat-path file must not be linked to a workflow run");

    pool.close().await;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&store_dir).ok();
}


// ─────────────────────────────────────────────────────────────────────────────
// External-server HTTP resource_link SSRF policy (same-host trust + env opt-in).
// A loopback mock stands in for a same-host (private/RFC1918) MCP artifact server.
// ─────────────────────────────────────────────────────────────────────────────

/// 200 response with a 12-byte CSV body (`a,b,c\n1,2,3\n`).
const OK_CSV_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Length: 12\r\n\r\na,b,c\n1,2,3\n";
/// 302 redirect to a DIFFERENT host (an off-host redirect the scoped path must not follow).
const REDIRECT_RESPONSE: &str =
    "HTTP/1.1 302 Found\r\nLocation: http://10.9.9.9:9/elsewhere.csv\r\nContent-Length: 0\r\n\r\n";

/// A loopback HTTP server that answers every request with `response`. Returns its
/// `http://127.0.0.1:<port>` base URL. The accept loop lives for the process (test-scoped).
async fn start_fixed_response_mock(response: &'static str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                // Read (and discard) the request head; we answer the same way regardless.
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });
    format!("http://{addr}")
}

/// Shared setup: TestServer + a user + process-global Repos/file-store pointed at this DB.
async fn setup_ingest_env(server: &TestServer, tag: &str) -> (Uuid, sqlx::PgPool, std::path::PathBuf) {
    let uid = user_id(server).await;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_{tag}_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);
    (uid, pool, store_dir)
}

/// TEST-6: matched host — an external link on a trusted (loopback stand-in) host is ingested,
/// and the link gets its file_id/version stamped back (the display-fix precondition).
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn http_link_matched_trusted_host_is_ingested() {
    let server = TestServer::start().await;
    let (uid, pool, store_dir) = setup_ingest_env(&server, "match").await;
    let base = start_fixed_response_mock(OK_CSV_RESPONSE).await;

    let mut links = vec![ziee_link(&format!("{base}/results/de_ad_control_limma.csv"), "de.csv")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "mcp",
        None,
        Uuid::new_v4(), // external/user server id
        false,          // NOT built-in → the SSRF-confined external branch
        &serde_json::json!({}),
        &["127.0.0.1".to_string()], // trusted_hosts: the mock's host → same-host trust
        &[],
        Some("test-secret"), // jwt_secret must be Some or the HTTP branch is skipped
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");

    assert_eq!(outcome.saved.len(), 1, "trusted-host artifact ingested");
    assert_eq!(outcome.saved[0].size, 12, "the 12-byte CSV body was actually fetched + saved");
    assert!(links[0].file_id.is_some(), "file_id stamped back → UI renders /api/files/{{id}}");

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}

/// TEST-7: unmatched host + env off — the default PUBLIC policy blocks the loopback host,
/// nothing is saved, and the link keeps its raw uri (no file_id).
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn http_link_unmatched_host_is_rejected() {
    let server = TestServer::start().await;
    let (uid, pool, store_dir) = setup_ingest_env(&server, "nomatch").await;
    let base = start_fixed_response_mock(OK_CSV_RESPONSE).await;
    let uri = format!("{base}/results/x.csv");

    let mut links = vec![ziee_link(&uri, "x.csv")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "mcp",
        None,
        Uuid::new_v4(),
        false,
        &serde_json::json!({}),
        &[], // trusted_hosts: EMPTY → not same-host → PUBLIC policy blocks loopback/RFC1918
        &[],
        Some("test-secret"),
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");

    assert_eq!(outcome.saved.len(), 0, "untrusted private host rejected by SSRF policy");
    assert_eq!(links[0].uri, uri, "rejected link keeps its original uri");
    assert!(links[0].file_id.is_none());

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}

/// TEST-8: release env opt-in — ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1 ingests the private link
/// even with an EMPTY trusted-host set (host-match not required).
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn http_link_env_optin_allows_private() {
    let server = TestServer::start().await;
    let (uid, pool, store_dir) = setup_ingest_env(&server, "envoptin").await;
    let base = start_fixed_response_mock(OK_CSV_RESPONSE).await;

    const KEY: &str = "ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE";
    let saved_env = std::env::var(KEY).ok();
    // SAFETY: this test is `serial(repos, ...)` and no other test reads this var.
    unsafe { std::env::set_var(KEY, "1") };

    let mut links = vec![ziee_link(&format!("{base}/results/x.csv"), "x.csv")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "mcp",
        None,
        Uuid::new_v4(),
        false,
        &serde_json::json!({}),
        &[], // trusted_hosts EMPTY — the env opt-in alone must permit the fetch
        &[],
        Some("test-secret"),
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");

    // SAFETY: restore before asserting so a panic can't leak the var to later tests.
    unsafe {
        match saved_env {
            Some(v) => std::env::set_var(KEY, v),
            None => std::env::remove_var(KEY),
        }
    }

    assert_eq!(outcome.saved.len(), 1, "env opt-in permits the private fetch");
    assert!(links[0].file_id.is_some());

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}

/// TEST-9: off-host redirect on the scoped path — the trusted-host fetch has redirects DISABLED,
/// so a 302 to a different host is not followed and nothing is saved.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn http_link_scoped_path_does_not_follow_offhost_redirect() {
    let server = TestServer::start().await;
    let (uid, pool, store_dir) = setup_ingest_env(&server, "redirect").await;
    let base = start_fixed_response_mock(REDIRECT_RESPONSE).await;

    let mut links = vec![ziee_link(&format!("{base}/results/x.csv"), "x.csv")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "mcp",
        None,
        Uuid::new_v4(),
        false,
        &serde_json::json!({}),
        &["127.0.0.1".to_string()], // trusted → PrivateScoped → redirects DISABLED
        &[],
        Some("test-secret"),
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");

    assert_eq!(outcome.saved.len(), 0, "off-host redirect is not followed → nothing saved");
    assert!(links[0].file_id.is_none());

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}
