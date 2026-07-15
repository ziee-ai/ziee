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
    // (was not orphan-deleted) and carries the run id. The file<->run link now lives in the
    // `file_workflow_runs` join table (chunk `ziee-file`: the store carries no run column).
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files WHERE id = $1")
        .bind(art.file_id)
        .fetch_one(&pool)
        .await
        .expect("ingested file row survives (not orphaned)");
    assert_eq!(exists, 1, "ingested resource_link file row survives");
    let linked_run: Option<Uuid> = sqlx::query_scalar(
        "SELECT workflow_run_id FROM file_workflow_runs WHERE file_id = $1",
    )
    .bind(art.file_id)
    .fetch_optional(&pool)
    .await
    .expect("query join table");
    assert_eq!(
        linked_run,
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

    // It is NOT linked to any workflow run (chat path) — no `file_workflow_runs` join row.
    let run_id: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM file_workflow_runs WHERE file_id = $1")
            .bind(art.file_id)
            .fetch_optional(&pool)
            .await
            .expect("query join table");
    assert!(run_id.is_none(), "chat-path file must not be linked to a workflow run");

    pool.close().await;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&store_dir).ok();
}


// ─────────────────────────────────────────────────────────────────────────────
// External-server HTTP resource_link SSRF policy (same-host trust).
// A loopback mock stands in for a same-host (private/RFC1918) MCP artifact server.
// (The release env opt-in ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE is proven purely in the
// resource_link.rs unit tests — mutating that process-global var here would race the parallel
// integration binary and could leak into concurrently-spawned server subprocesses.)
// ─────────────────────────────────────────────────────────────────────────────

/// 200 response with a 12-byte CSV body (`a,b,c\n1,2,3\n`).
const OK_CSV_RESPONSE: &str =
    "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Length: 12\r\n\r\na,b,c\n1,2,3\n";

/// Aborts a mock server's accept loop when dropped, so a test doesn't leak a live accept task
/// for the rest of the test-process lifetime. Each test binds it to a `_mock` local.
struct MockGuard(tokio::task::JoinHandle<()>);
impl Drop for MockGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A loopback HTTP server that answers every request with `response`. Returns its
/// `http://127.0.0.1:<port>` base URL and a guard that stops the accept loop on drop.
async fn start_fixed_response_mock(response: impl Into<String>) -> (String, MockGuard) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let response = std::sync::Arc::new(response.into());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let response = response.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                // Read (and discard) the request head; we answer the same way regardless.
                let _ = sock.read(&mut buf).await;
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });
    (format!("http://{addr}"), MockGuard(handle))
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
    let (base, _mock) = start_fixed_response_mock(OK_CSV_RESPONSE).await;

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
    let (base, _mock) = start_fixed_response_mock(OK_CSV_RESPONSE).await;
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

/// TEST-9: the scoped (same-host trust) path has redirects DISABLED. The mock 302-redirects to a
/// SECOND, reachable loopback mock that serves 200. If redirects were followed the client would
/// reach that 200 target (loopback → allowed by MCP_USER) and save the artifact; because they are
/// disabled, the 302 itself is a non-success response and nothing is saved. This DISTINGUISHES
/// redirects-disabled from redirects-followed (the target is reachable + successful), so the test
/// would fail if the redirect-disabling were reverted.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn http_link_scoped_path_does_not_follow_redirect() {
    let server = TestServer::start().await;
    let (uid, pool, store_dir) = setup_ingest_env(&server, "redirect").await;

    // The redirect TARGET is a reachable loopback mock serving a real 200 body.
    let (target, _target_mock) = start_fixed_response_mock(OK_CSV_RESPONSE).await;
    // The primary mock 302-redirects to that target.
    let redirect_resp = format!(
        "HTTP/1.1 302 Found\r\nLocation: {target}/redirected.csv\r\nContent-Length: 0\r\n\r\n"
    );
    let (base, _redir_mock) = start_fixed_response_mock(redirect_resp).await;

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

    assert_eq!(
        outcome.saved.len(),
        0,
        "redirect is not followed (would be 1 if the scoped path followed it to the 200 target)"
    );
    assert!(links[0].file_id.is_none());

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}

/// TEST-4 (stale-artifact-links): re-fetching an artifact link yields the CURRENT file,
/// never a stale one. This is the behavioral premise the corrected sandbox guidance steers
/// the model toward — when it re-obtains a link (via get_resource_link → ziee:// →
/// persist_links) instead of reusing an earlier-turn URL, the fresh ingest reads the live
/// workspace bytes. Prove it directly on the ingest tail: two persist_links rounds over the
/// SAME host path across an overwrite produce two distinct files whose blobs are v1 then v2.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn refetch_reingest_yields_current_file_not_stale() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_refetch_store_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);

    // A per-conversation workspace root with an artifact the sandbox "produced".
    let root = std::env::temp_dir().join(format!("ziee_rl_refetch_ws_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("gene_stats.csv");
    let v1 = b"gene,logFC\nA,1.0\n";
    std::fs::write(&artifact, v1).unwrap();

    // Round 1 (turn 1): produce a link for the current file and ingest it.
    let mut links1 = vec![ziee_link(&format!("ziee://{}", artifact.display()), "gene_stats.csv")];
    let out1 = ziee::persist_links(
        &mut links1,
        uid,
        None,
        None,
        "mcp",
        None,
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[],
        &[root.clone()],
        None,
        None,
        None,
    )
    .await
    .expect("persist_links round 1");
    assert_eq!(out1.saved.len(), 1, "round 1 ingests the current file");
    let file_v1 = out1.saved[0].file_id;
    let blob_v1 = ziee::get_file_storage()
        .load_original(uid, file_v1, "csv")
        .await
        .expect("v1 blob readable");
    assert_eq!(blob_v1, v1, "round 1 blob is the v1 bytes");

    // The workspace file changes (a later DE re-run overwrites gene_stats.csv).
    let v2 = b"gene,logFC\nA,2.5\nB,-1.1\n";
    std::fs::write(&artifact, v2).unwrap();

    // Round 2 (a LATER turn): re-obtaining a link and re-ingesting must read the NEW bytes,
    // not the stale v1 — a distinct file whose blob is v2.
    let mut links2 = vec![ziee_link(&format!("ziee://{}", artifact.display()), "gene_stats.csv")];
    let out2 = ziee::persist_links(
        &mut links2,
        uid,
        None,
        None,
        "mcp",
        None,
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[],
        &[root.clone()],
        None,
        None,
        None,
    )
    .await
    .expect("persist_links round 2");
    assert_eq!(out2.saved.len(), 1, "round 2 ingests the current file");
    let file_v2 = out2.saved[0].file_id;
    assert_ne!(file_v1, file_v2, "re-fetch produces a distinct file, not the stale one");
    let blob_v2 = ziee::get_file_storage()
        .load_original(uid, file_v2, "csv")
        .await
        .expect("v2 blob readable");
    assert_eq!(blob_v2, v2, "round 2 blob is the CURRENT v2 bytes, never stale");

    pool.close().await;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&store_dir).ok();
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin-registered SYSTEM MCP server re-hosting (the fix).
//
// An admin-registered system server (`is_system=true, is_built_in=false`) carries a REAL external
// `url` (e.g. `host.docker.internal:<port>`) whose host is REDACTED in the user-facing list. The
// new `list_accessible_result_link_hosts` accessor recovers that host (server-side, hosts only) so
// its result files get re-hosted — while a built-in (loopback) server's host stays excluded. These
// exercise the REDACTION BYPASS the fix adds, which the direct-`trusted_hosts` unit/prior tests
// can't reach. A loopback mock stands in for the private artifact host in the ingest test.
// ─────────────────────────────────────────────────────────────────────────────

const MCP_ADMIN_PERMS: &[&str] = &["mcp_servers_admin::create", "mcp_servers_admin::read"];

/// Register a SYSTEM MCP server (`is_system=true, is_built_in=false`) at `url` via the admin API,
/// then grant it to the default group so `admin` (a default-group member) can access it. Returns
/// the new server id. Mirrors the create+grant pattern in `mcp_sampling_test`.
async fn register_system_server(
    server: &TestServer,
    admin: &test_helpers::TestUser,
    url: &str,
) -> Uuid {
    let payload = serde_json::json!({
        "name": format!("sys_{}", &Uuid::new_v4().to_string()[..8]),
        "display_name": "Org System Server",
        "description": "system server for resource_link rehost test",
        "enabled": true,
        "transport_type": "http",
        "url": url,
        "usage_mode": "auto",
        "timeout_seconds": 120
    });
    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("create system server");
    let status = resp.status();
    let body_txt = resp.text().await.unwrap_or_default();
    assert_eq!(status, 201, "system-server create should 201: {body_txt}");
    let body: serde_json::Value = serde_json::from_str(&body_txt).unwrap();
    let server_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // Grant to the default group (registered users are members) so the accessor sees it.
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let dg = sqlx::query!("SELECT id FROM groups WHERE is_default = true LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("default group exists");
    sqlx::query!(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) VALUES ($1, $2, NOW())",
        dg.id,
        server_id,
    )
    .execute(&pool)
    .await
    .expect("grant system server to default group");
    // Force enabled=true: `create_system_server` runs a health probe and auto-downgrades to
    // enabled=false when the probe fails (our fixture host isn't a real MCP server). The trust-host
    // derivation intentionally only trusts ENABLED servers, so re-assert enabled to model a normal
    // operational server — the probe-downgrade is orthogonal to what these tests exercise.
    sqlx::query!("UPDATE mcp_servers SET enabled = true WHERE id = $1", server_id)
        .execute(&pool)
        .await
        .expect("re-enable system server after create-time probe downgrade");
    pool.close().await;
    server_id
}

/// TEST-3: a registered SYSTEM server's result file is re-hosted. The user-facing list redacts the
/// server's url, yet the trust-host accessor recovers its host, so `persist_links` ingests an
/// external link on that host under MCP_USER and stamps `file_id`. The negative control proves an
/// unregistered private host stays blocked (no blanket private allow).
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn system_server_host_is_trusted_and_result_file_ingested() {
    let server = TestServer::start().await;
    // Admin user creates the system server AND is a default-group member, so it can access it.
    let admin = test_helpers::create_user_with_permissions(&server, "rladmin", MCP_ADMIN_PERMS).await;
    let uid = Uuid::parse_str(&admin.user_id).unwrap();

    // Point the in-process Repos + file store at THIS test's DB.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    ziee::init_repositories(pool.clone());
    let store_dir = std::env::temp_dir().join(format!("ziee_rl_sys_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&store_dir).unwrap();
    ziee::init_file_storage(&store_dir);

    // A loopback mock stands in for the private (host.docker.internal) artifact host.
    let (base, _mock) = start_fixed_response_mock(OK_CSV_RESPONSE).await; // http://127.0.0.1:<port>
    let server_id = register_system_server(&server, &admin, &format!("{base}/mcp")).await;

    // CONTROL: the user-facing list REDACTS the system server's url (its host would be lost there).
    let listed = ziee::Repos
        .mcp
        .list_accessible(uid, 1, 100, None, None, Some(true))
        .await
        .expect("list_accessible");
    let sysrow = listed
        .servers
        .iter()
        .find(|s| s.id == server_id)
        .expect("system server visible to the user");
    assert!(sysrow.url.is_none(), "user-facing list redacts the system server url");

    // ...but the trust-host accessor RECOVERS its host (the redaction bypass this fix adds).
    let trusted = ziee::Repos
        .mcp
        .list_accessible_result_link_hosts(uid)
        .await
        .expect("list_accessible_result_link_hosts");
    assert!(
        trusted.iter().any(|h| h == "127.0.0.1"),
        "system server host is in the trust set: {trusted:?}"
    );

    // End-to-end: an external result-file link on that host is now ingested under MCP_USER.
    let mut links = vec![ziee_link(&format!("{base}/results/analysis.csv"), "analysis.csv")];
    let outcome = ziee::persist_links(
        &mut links,
        uid,
        None,
        None,
        "mcp",
        None,
        server_id,
        false, // external branch (not built-in)
        &serde_json::json!({}),
        &trusted,
        &[],
        Some("test-secret"),
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");
    assert_eq!(outcome.saved.len(), 1, "system-server result file ingested");
    assert_eq!(outcome.saved[0].size, 12, "the 12-byte CSV body was fetched + saved");
    assert!(links[0].file_id.is_some(), "file_id stamped → UI renders /api/files/{{id}}");

    // NEGATIVE: the same bytes on a host NOT in the trust set stay blocked (no blanket allow).
    let mut untrusted = vec![ziee_link(&format!("{base}/results/x.csv"), "x.csv")];
    let out2 = ziee::persist_links(
        &mut untrusted,
        uid,
        None,
        None,
        "mcp",
        None,
        Uuid::new_v4(),
        false,
        &serde_json::json!({}),
        &[], // empty trust set → PUBLIC policy blocks the loopback/private host
        &[],
        Some("test-secret"),
        Some("ziee"),
        Some("ziee-api"),
    )
    .await
    .expect("persist_links");
    assert_eq!(out2.saved.len(), 0, "an unregistered private host stays blocked");

    pool.close().await;
    std::fs::remove_dir_all(&store_dir).ok();
}

/// TEST-4: the trust-host accessor returns an admin-registered non-built-in system server's host
/// (redaction bypassed), and OMITS it once the same row is flipped to a built-in (in-process
/// loopback) server — the loopback-SSRF exclusion holding through the accessor.
#[tokio::test]
#[serial_test::serial(repos, file_storage)]
async fn accessor_returns_system_host_and_omits_builtin() {
    let server = TestServer::start().await;
    let admin =
        test_helpers::create_user_with_permissions(&server, "rladmin2", MCP_ADMIN_PERMS).await;
    let uid = Uuid::parse_str(&admin.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&server.database_url)
        .await
        .unwrap();
    ziee::init_repositories(pool.clone());

    let server_id =
        register_system_server(&server, &admin, "http://host.docker.internal:18122/mcp").await;

    // is_built_in=false (as created) → the accessor recovers the (redacted-in-list) host.
    let hosts = ziee::Repos
        .mcp
        .list_accessible_result_link_hosts(uid)
        .await
        .expect("accessor");
    assert!(
        hosts.iter().any(|h| h == "host.docker.internal"),
        "an admin-registered non-built-in system host is trusted (redaction bypassed): {hosts:?}"
    );

    // The shared derivation used by ALL 3 persist_links call sites (chat approval, chat auto-exec,
    // workflow dispatch): an EXTERNAL emitter gets the registered system host; a BUILT-IN emitter
    // short-circuits to an empty set (skips the DB query). This covers the call-site glue logic that
    // the direct persist_links/accessor calls bypass.
    let via_external = ziee::result_link_trusted_hosts(false, uid).await;
    assert!(
        via_external.iter().any(|h| h == "host.docker.internal"),
        "external emitter → registered system host in the trust set: {via_external:?}"
    );
    let via_builtin = ziee::result_link_trusted_hosts(true, uid).await;
    assert!(
        via_builtin.is_empty(),
        "built-in emitter → empty trust set (no query): {via_builtin:?}"
    );

    // Flip the row to a built-in (in-process loopback) server → the accessor must OMIT its host.
    sqlx::query!(
        "UPDATE mcp_servers SET is_built_in = true WHERE id = $1",
        server_id
    )
    .execute(&pool)
    .await
    .expect("flip is_built_in");
    let hosts2 = ziee::Repos
        .mcp
        .list_accessible_result_link_hosts(uid)
        .await
        .expect("accessor");
    assert!(
        !hosts2.iter().any(|h| h == "host.docker.internal"),
        "a built-in (loopback) server's host must be excluded from the trust set: {hosts2:?}"
    );

    pool.close().await;
}
