//! Integration tests for the shared `mcp::resource_link::persist_links` consumer.
//!
//! Driven by calling the re-exported `ziee::persist_links` directly in-process — the same
//! way the chat extension + workflow dispatcher invoke it internally. Tier-2 (Postgres +
//! file store). Run with `--test-threads=1` (the project default).
//!
//! NOTE on process-global state: `persist_links`' ingest tail uses the process-global
//! `Repos` (`init_repositories`, first-call-wins) and `FILE_STORAGE` (`init_file_storage`,
//! idempotent). `TestServer` runs the server in a SEPARATE process, so neither global is
//! set in THIS test process. Exactly ONE test here (`persist_ingests_*`) initializes them
//! in-process; it is the sole `init_repositories` caller in the whole integration binary
//! (no other integration test uses the global `Repos` in-process), so the global-once
//! factory safely points at that test's DB for the duration. Every other test exercises a
//! guard short-circuit that returns BEFORE the save tail, so it never touches the globals.

use crate::common::{test_helpers, TestServer};
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
///  - `workflow_run_id = Some(..)` is accepted (documented no-op on this branch)
#[tokio::test]
async fn persist_ingests_ziee_under_root_and_handles_mixed_links() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    // Point the process-global Repos + file store at THIS test's DB + a temp dir.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    if !ziee::is_repos_initialized() {
        ziee::init_repositories(pool.clone());
    }
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
        Some(Uuid::new_v4()), // workflow_run_id accepted (no-op on this branch)
        code_sandbox_server_id(),
        true,
        &serde_json::json!({}),
        &[root.clone()],
        None,
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

/// Chat path (NOT a workflow run): a code_sandbox `ziee://` artifact saved
/// during a chat passes `workflow_run_id = None`, so the ingested file must be
/// created but NOT linked to any run (`files.workflow_run_id IS NULL`) — the
/// counterpart to the workflow-path test above, which links the file to a run.
/// This pins the code_sandbox → file-store integration for the chat dispatcher.
#[tokio::test]
async fn code_sandbox_chat_path_persists_artifact_without_run_link() {
    let server = TestServer::start().await;
    let uid = user_id(&server).await;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&server.database_url)
        .await
        .unwrap();
    if !ziee::is_repos_initialized() {
        ziee::init_repositories(pool.clone());
    }
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
        &[root.clone()],
        None,
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
        &[root.clone()],
        None,
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
        &[],
        None,
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
        &[root.clone()],
        None,
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
        &[],
        None, // no jwt secret → HTTP branch is skipped (the dispatcher path)
    )
    .await
    .expect("persist_links");

    assert!(
        outcome.saved.is_empty(),
        "HTTP link must be skipped without a jwt secret"
    );
    assert_eq!(links[0].uri, url, "a non-ziee:// URI is left untouched");
}
