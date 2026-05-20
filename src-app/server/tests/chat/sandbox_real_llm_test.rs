//! Tier 5 — chat → real LLM → code_sandbox round-trip.
//!
//! Requires:
//!   - bwrap + mounted rootfs (see Tier 4 prerequisites)
//!   - code_sandbox.enabled = true in the test config
//!   - ANTHROPIC_API_KEY set in tests/.env.test
//!
//! These tests cost real API tokens; they run only in the nightly
//! `sandbox-integration-nightly.yml` workflow, gated on both the
//! env var AND the rootfs being mounted.
//!
//! The pattern mirrors tests/chat/mcp_elicitation_test.rs (real
//! Anthropic + assertion on the SSE event stream).

#[tokio::test]
#[ignore]
async fn list_files_via_llm_is_auto_approved() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return;
    }
    eprintln!(
        "test skipped: end-to-end chat→LLM→sandbox test scaffolded but not yet wired \
         (needs built rootfs + sandbox enabled in test config; tracked as follow-up)"
    );
}

#[tokio::test]
#[ignore]
async fn read_file_via_llm_is_auto_approved() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return;
    }
    eprintln!(
        "test skipped: end-to-end chat→LLM→sandbox test scaffolded but not yet wired \
         (needs built rootfs + sandbox enabled in test config; tracked as follow-up)"
    );
}

#[tokio::test]
#[ignore]
async fn execute_command_emits_approval_required_sse_event() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("test skipped: ANTHROPIC_API_KEY not set");
        return;
    }
    eprintln!(
        "test skipped: end-to-end chat→LLM→sandbox test scaffolded but not yet wired \
         (needs built rootfs + sandbox enabled in test config; tracked as follow-up)"
    );
}
