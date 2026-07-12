//! Unit tests for `group_assistant_blocks` — the pure wire-format reconstruction
//! that turns one assistant message's stored content blocks into provider-ready
//! per-iteration `[Assistant {text + tool_use}, Tool {tool_result}]` pairs.
//!
//! This is the consumer side of Fix A (atomic `sequence_order`): given blocks in
//! `sequence_order`, every tool_use in an Assistant turn must be resolved by a
//! tool_result in the immediately following Tool turn — violating that is what
//! surfaces as the Anthropic error "tool_use should have tool_result blocks".
//!
//! Exercised via the doc-hidden `ziee::test_internals` re-exports (the function
//! and the `ai_providers` wire types are otherwise private to the lib).

use serde_json::json;
use ziee::test_internals::{group_assistant_blocks, ChatMessage, ContentBlock, Role};

fn tool_use(id: &str, name: &str) -> ContentBlock {
    ContentBlock::ToolUse {
        id: id.to_string(),
        name: name.to_string(),
        input: json!({}),
    }
}

fn tool_result(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        name: None,
        content: vec![ContentBlock::Text {
            text: content.to_string(),
        }],
        is_error: None,
    }
}

fn tool_use_ids(content: &[ContentBlock]) -> Vec<String> {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn tool_result_ids(content: &[ContentBlock]) -> Vec<String> {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
            _ => None,
        })
        .collect()
}

/// Assert the Anthropic invariant: every Assistant turn carrying tool_use blocks
/// is immediately followed by a Tool turn whose tool_results resolve EXACTLY those
/// ids, and no Tool turn is left orphaned.
fn assert_valid_tool_pairing(msgs: &[ChatMessage]) {
    let mut i = 0;
    while i < msgs.len() {
        let uses = tool_use_ids(&msgs[i].content);
        if matches!(msgs[i].role, Role::Assistant) && !uses.is_empty() {
            let next = msgs
                .get(i + 1)
                .unwrap_or_else(|| panic!("tool_use turn at {i} not followed by a Tool turn"));
            assert!(
                matches!(next.role, Role::Tool),
                "message after a tool_use turn must be a Tool turn, got {:?}",
                next.role
            );
            let mut got = tool_result_ids(&next.content);
            let mut want = uses.clone();
            got.sort();
            want.sort();
            assert_eq!(
                got, want,
                "tool_result ids must resolve exactly the preceding tool_use ids"
            );
            i += 2;
        } else {
            assert!(
                !matches!(msgs[i].role, Role::Tool),
                "orphaned Tool turn at index {i} (no preceding tool_use turn)"
            );
            i += 1;
        }
    }
}

/// The user's exact scenario, in the CORRECT (post-fix) monotonic order:
/// two parallel write_file tool_uses, both results, then execute_command.
/// Must reconstruct into two clean iteration pairs.
#[test]
fn parallel_tool_uses_then_another_tool_use_groups_per_iteration() {
    let blocks = vec![
        tool_use("w1", "write_file"),
        tool_use("w2", "write_file"),
        tool_result("w1", "ok"),
        tool_result("w2", "ok"),
        tool_use("exec", "execute_command"),
        tool_result("exec", "ran"),
    ];

    let msgs = group_assistant_blocks(blocks);

    assert_valid_tool_pairing(&msgs);
    assert_eq!(msgs.len(), 4, "two iterations → 2 Assistant + 2 Tool turns");
    assert!(matches!(msgs[0].role, Role::Assistant));
    assert_eq!(tool_use_ids(&msgs[0].content), vec!["w1", "w2"]);
    assert!(matches!(msgs[1].role, Role::Tool));
    assert_eq!(tool_result_ids(&msgs[1].content), vec!["w1", "w2"]);
    assert!(matches!(msgs[2].role, Role::Assistant));
    assert_eq!(tool_use_ids(&msgs[2].content), vec!["exec"]);
    assert!(matches!(msgs[3].role, Role::Tool));
    assert!(!matches!(msgs.last().unwrap().role, Role::Assistant));
}

/// Robustness guard: even fed the CORRUPTED interleaving the old sequence_order
/// bug produced (execute_command's tool_use sorted BETWEEN the two parallel
/// write_file results), every tool_use must still end up resolved by a
/// tool_result. (The repository fix prevents this ordering; this proves the
/// consumer is not also fragile.)
#[test]
fn corrupted_interleaving_still_pairs_every_tool_use() {
    let blocks = vec![
        tool_use("w1", "write_file"),
        tool_use("w2", "write_file"),
        tool_result("w1", "ok"),
        tool_use("exec", "execute_command"), // collided seq → sorted mid-results
        tool_result("w2", "ok"),
        tool_result("exec", "ran"),
    ];

    let msgs = group_assistant_blocks(blocks);

    assert_valid_tool_pairing(&msgs);
}

/// Approval flow: a lone tool_use with no result yet is emitted as a single
/// trailing Assistant turn (before_llm_call appends the result as a following
/// User message later).
#[test]
fn trailing_tool_use_without_result_is_emitted_as_assistant() {
    let msgs = group_assistant_blocks(vec![tool_use("a", "search")]);

    assert_eq!(msgs.len(), 1);
    assert!(matches!(msgs[0].role, Role::Assistant));
    assert_eq!(tool_use_ids(&msgs[0].content), vec!["a"]);
}

/// TEST-5a: the failed-parallel repro — three parallel tool_uses but only ONE real
/// result captured (the others failed/absent). group_assistant_blocks must still
/// produce a VALID pairing: the real result plus a synthesized result for each
/// missing id, so no tool_use is left dangling (which every provider 400s on).
#[test]
fn partial_parallel_batch_synthesizes_missing_and_stays_valid() {
    let blocks = vec![
        tool_use("A", "srv__run_pathway_analysis"),
        tool_use("B", "srv__run_consensus_analysis"),
        tool_use("C", "srv__run_consensus_analysis"),
        tool_result("A", "ok-A"),
    ];
    let msgs = group_assistant_blocks(blocks);

    assert_valid_tool_pairing(&msgs);
    assert_eq!(msgs.len(), 2, "one Assistant/Tool pair");
    assert_eq!(tool_use_ids(&msgs[0].content), vec!["A", "B", "C"]);
    assert_eq!(
        tool_result_ids(&msgs[1].content),
        vec!["A", "B", "C"],
        "every tool_use id is answered in the following Tool turn"
    );
}

/// TEST-5b: a multi-iteration agentic loop merged into ONE stored assistant message
/// (iter1: one tool; iter2: two parallel tools) reconstructs into valid per-iteration
/// pairs across every boundary.
#[test]
fn multi_iteration_single_message_stays_valid() {
    let blocks = vec![
        tool_use("i1", "search"),
        tool_result("i1", "r1"),
        tool_use("i2a", "read_file"),
        tool_use("i2b", "read_file"),
        tool_result("i2a", "ra"),
        tool_result("i2b", "rb"),
    ];
    let msgs = group_assistant_blocks(blocks);

    assert_valid_tool_pairing(&msgs);
    assert_eq!(msgs.len(), 4, "two iterations → 2 Assistant + 2 Tool turns");
    assert_eq!(tool_use_ids(&msgs[0].content), vec!["i1"]);
    assert_eq!(tool_use_ids(&msgs[2].content), vec!["i2a", "i2b"]);
}
