//! Conversation auto-title generation (the `title-generation` chat extension).
//!
//! Regression coverage for the production bug where a chat through a REASONING
//! model (`openai/gpt-oss-120b`) was permanently titled with the RAW first user
//! message. Root cause: the title request's 50-token budget was consumed
//! entirely by the model's `reasoning_content` preamble, the stream ended with
//! `finish_reason: "length"` having emitted no answer text, and the extension
//! fell back to `generate_simple_title` — the first 50 characters of the user's
//! own message — which the "title already set" guard then made permanent.
//!
//! The fix: a reasoning-safe token budget, and an empty generation leaves the
//! title UNSET (so a later turn retries) instead of persisting the raw message.

use crate::chat::helpers;
use crate::common::stub_chat::{
    self, STUB_TITLE, STUB_TITLE_EMPTY, STUB_TITLE_EMPTY_ONCE, StubChat,
};
use uuid::Uuid;

/// Spin up a server + a stub-backed model and return everything a title test
/// needs. `tools` controls whether the model advertises tool capability.
async fn setup(
    label: &str,
    tools: bool,
) -> (
    crate::common::TestServer,
    crate::common::test_helpers::TestUser,
    StubChat,
    Uuid,
) {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, label, &["*"]).await;
    let stub = StubChat::start().await;
    let model_id = stub_chat::register_stub_model(
        &server,
        &user.token,
        &user.user_id,
        &stub.base_url,
        tools,
        None,
    )
    .await;
    let model_id = Uuid::parse_str(&model_id).unwrap();
    (server, user, stub, model_id)
}

/// Create an untitled conversation, send one message, return `(conversation, branch)`.
async fn first_exchange(
    server: &crate::common::TestServer,
    token: &str,
    model_id: Uuid,
    content: &str,
) -> (Uuid, Uuid) {
    let conv = helpers::create_conversation(server, token, Some(model_id), None).await;
    let conv_id = helpers::parse_uuid(&conv["id"]);
    let branch_id = helpers::parse_uuid(&conv["active_branch_id"]);
    assert!(
        conv["title"].is_null(),
        "a conversation must start untitled so the extension is what sets the title"
    );
    helpers::send_and_collect(server, token, conv_id, branch_id, model_id, content).await;
    (conv_id, branch_id)
}

/// Count of title-generation calls the stub has received.
fn title_call_count(stub: &StubChat) -> usize {
    stub.requests().iter().filter(|r| r.is_title_request).count()
}

/// The stored title.
///
/// No polling needed: `call_after_llm_call` (and therefore the title LLM call
/// and the title write) is awaited inside `finalize()` BEFORE the terminal
/// `complete` frame that `send_and_collect` returns on.
async fn stored_title(
    server: &crate::common::TestServer,
    token: &str,
    conv_id: Uuid,
) -> Option<String> {
    let conv = helpers::get_conversation(server, token, conv_id).await;
    conv["title"].as_str().map(|s| s.to_string())
}

/// TEST-9 — the cross-model regression guard.
///
/// A NORMAL, non-reasoning model that returns text on the first exchange must
/// still get an AI-generated title, written exactly once. This is the path that
/// already worked in production; the budget bump and the empty→unset change must
/// not regress it.
#[tokio::test]
async fn normal_model_gets_an_ai_generated_title_on_the_first_exchange() {
    let (server, user, stub, model_id) = setup("title_normal", false).await;

    let (conv_id, branch_id) =
        first_exchange(&server, &user.token, model_id, "How do I sort a list?").await;

    let title = stored_title(&server, &user.token, conv_id)
        .await
        .expect("a non-reasoning model must produce a title on the first exchange");
    assert_eq!(
        title, STUB_TITLE,
        "the title must come from the AI title call"
    );
    assert_ne!(
        title, "How do I sort a list?",
        "the title must NOT be the raw first user message"
    );
    assert_eq!(title_call_count(&stub), 1, "one title call on turn 1");

    // A SECOND turn must not re-generate. The fix replaced a hard single-shot
    // count guard with a retry predicate, so "exactly once" is only meaningful
    // if a later turn is actually sent — asserting it after one turn is vacuous.
    helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "and in reverse?",
    )
    .await;

    assert_eq!(
        title_call_count(&stub),
        1,
        "an already-titled conversation must not pay for another title call"
    );
    assert_eq!(
        stored_title(&server, &user.token, conv_id).await.as_deref(),
        Some(STUB_TITLE),
        "the title must be stable across turns"
    );
}

/// TEST-12 — the budget reaches the wire.
///
/// Pins the root-cause fix end-to-end: the title request the provider actually
/// sent must carry the reasoning-safe budget, not the old starving 50.
#[tokio::test]
async fn title_request_carries_the_reasoning_safe_token_budget() {
    let (server, user, stub, model_id) = setup("title_budget", false).await;

    let _ = first_exchange(&server, &user.token, model_id, "Explain TCP slow start").await;

    let title_request = stub
        .requests()
        .into_iter()
        .find(|r| r.is_title_request)
        .expect("the title extension must have issued a generation call");
    assert_eq!(
        title_request.max_tokens,
        Some(512),
        "the title budget must be large enough for a reasoning model's preamble"
    );
    assert!(
        title_request.tool_names.is_empty(),
        "the title call must be tool-less"
    );
}

/// TEST-10 — the direct regression test for the reported bug.
///
/// When the model returns an EMPTY completion for the title call (exactly what
/// gpt-oss-120b did after burning its budget on hidden reasoning), the
/// conversation must be left UNTITLED — never titled with the raw user message.
#[tokio::test]
async fn an_empty_generation_leaves_the_title_unset_not_the_raw_message() {
    let (server, user, stub, model_id) = setup("title_empty", false).await;

    // The token rides in the user message; the title prompt quotes it verbatim,
    // so the stub sees it on the title call and answers with no text.
    let content = format!("What is known about BRCA1 in breast cancer? {STUB_TITLE_EMPTY}");
    let (conv_id, branch_id) = first_exchange(&server, &user.token, model_id, &content).await;

    assert_eq!(
        title_call_count(&stub),
        1,
        "the extension must have ATTEMPTED generation (otherwise this proves nothing)"
    );
    assert_eq!(
        stored_title(&server, &user.token, conv_id).await,
        None,
        "an empty generation must leave the title UNSET — never the raw user message"
    );

    // The next turn must RETRY — impossible under the old `message_count != 2`
    // guard, which stranded a turn-1 failure permanently untitled.
    helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "summarize that for me",
    )
    .await;

    assert_eq!(
        title_call_count(&stub),
        2,
        "an untitled conversation must retry generation on the next turn"
    );
    // Still empty here by construction: the title prompt always quotes the
    // conversation's FIRST user message, which still carries the token. The
    // retry SUCCEEDING is covered by the transient test below.
    assert_eq!(
        stored_title(&server, &user.token, conv_id).await,
        None,
        "a persistently empty generation must still never store the raw message"
    );
}

/// The retry actually SUCCEEDS on a transient failure.
///
/// This is the behavior the old `message_count != 2` guard made impossible: once
/// the user+assistant count moved past 2, a conversation that failed to generate
/// on turn 1 could never be titled again. Combined with the old raw-message
/// fallback, that is the full shape of the reported bug.
#[tokio::test]
async fn a_transient_generation_failure_is_retried_and_then_succeeds() {
    let (server, user, stub, model_id) = setup("title_retry", false).await;

    let content = format!("What is known about BRCA1 in breast cancer? {STUB_TITLE_EMPTY_ONCE}");
    let (conv_id, branch_id) = first_exchange(&server, &user.token, model_id, &content).await;

    assert_eq!(
        stored_title(&server, &user.token, conv_id).await,
        None,
        "turn 1's empty generation must leave the title unset"
    );

    helpers::send_and_collect(
        &server,
        &user.token,
        conv_id,
        branch_id,
        model_id,
        "summarize that for me",
    )
    .await;

    assert_eq!(
        title_call_count(&stub),
        2,
        "the untitled conversation must have retried on turn 2"
    );
    assert_eq!(
        stored_title(&server, &user.token, conv_id).await.as_deref(),
        Some(STUB_TITLE),
        "the retry must succeed once the model returns text"
    );
}

/// TEST-11 — a TOOL-CAPABLE model still gets a title.
///
/// Scope note, deliberately narrow: this exercises the tool-capable model path
/// (tools advertised, the MCP extension engaged in `after_llm_call`) but the
/// stub answers with text on the first iteration, so it does NOT drive a
/// multi-iteration tool loop. The multi-iteration shape — where `tool_use` and
/// `tool_result` blocks accumulate on the SAME assistant row — is covered by
/// `fires_on_a_tool_calling_first_turn` in title.rs's unit tests (which builds
/// that exact row shape) and end-to-end by the live BioGnosia verification.
#[tokio::test]
async fn a_tool_capable_first_turn_still_gets_a_title() {
    let (server, user, stub, model_id) = setup("title_tools", true).await;

    let (conv_id, _) = first_exchange(
        &server,
        &user.token,
        model_id,
        "STUB_PLAN=read_first_file summarize the attachment",
    )
    .await;

    assert_eq!(
        title_call_count(&stub),
        1,
        "a tool-capable turn must still trigger title generation"
    );
    assert_eq!(
        stored_title(&server, &user.token, conv_id).await.as_deref(),
        Some(STUB_TITLE),
        "the title must be AI-generated, not the raw first user message"
    );
}
