# Audit — Round 8

Branch: `feat/project-improvements`
Worktree: `/home/pbya/projects/ziee-chat-feat-project-improvements`
Date: 2026-06-08

## Summary

| # | ID | Severity | Category | File | Title |
|---|----|----------|----------|------|-------|
| 1 | `r8-verify-r7-trackA-01` | High | regression | `src-app/server/src/modules/file/chat_extension/file.rs` | Empty-text push branch re-inlines the current upload as a trailing User turn on every tool-loop iteration ≥2 |

**Totals:** 1 confirmed — High: 1, Medium: 0, Low: 0.

**Status:** NOT CONVERGED (one High finding outstanding).

---

## Finding 1 — Empty-text push branch re-inlines the current upload as a trailing User turn on every tool-loop iteration ≥2

- **ID:** `r8-verify-r7-trackA-01`
- **Severity:** High
- **Category:** regression
- **File:** `src-app/server/src/modules/file/chat_extension/file.rs` (`before_llm_call`, lines 139–205)
- **Introduced:** `da0e77e6` ("fix(track-a): close round-7 edge cases in the replay division of labor") — the prior revision `ebf1e00c` had only the append-only `if last == User` arm, which is a no-op on iteration ≥2.

### What happens

`before_llm_call` runs on **every** tool-loop iteration, and `send_request`
(the streaming fn param threaded as `&request` from
`modules/chat/core/services/streaming.rs`) is never reset — so
`send_request.file_ids` is identical each iteration. `process_file_blocks`
re-reads the same non-image attachment, leaving `file_blocks` non-empty on
every pass.

- **Iteration 1 (correct):** the empty assistant placeholder is filtered out
  of the replay, so `request.messages.last()` is the user turn. The
  `if let Some(last_message) … last_message.role == Role::User` arm
  (file.rs:193–196) extends that user turn — the upload lands exactly once.
- **Iteration ≥2 (bug):** the assistant message now carries
  `tool_use` + `tool_result` and is **kept**. `group_assistant_blocks`
  flushes a completed round-trip as `[Assistant{tool_use}, Tool{tool_result}]`,
  so the **last** provider message is `Role::Tool` (the original finding said
  `Assistant`; it is actually `Tool` — either way it is not `User`, which
  strengthens the conclusion). The original user attachment was dropped by the
  recency rule and the manifest `System` message sits at index 0 only. With
  `last() != User`, the new `else if !file_blocks.is_empty()` branch
  (file.rs:197–202) **pushes a fresh `Role::User` message** carrying the
  re-inlined attachment **after** the assistant tool turn.

### Impact

- Defeats the manifest token-saving design — the attachment the manifest
  intends to leave out (recoverable via `read_file`) gets re-inlined in full
  on every iteration.
- Corrupts the `tool_use → tool_result → continuation` structure by injecting
  a stray `User` turn between the assistant tool round-trip and the model's
  continuation.
- Adds redundant per-iteration `process_file_blocks` DB work.

### Verification performed

- Code in the worktree matches the reported branch (file.rs:193–202).
- `before_llm_call(&self, context: &mut StreamContext, request, send_request, _tx)`
  receives `context`, and `StreamContext.iteration: u32` exists
  (`modules/chat/core/extension/registry.rs:88`, documented as 1-indexed).
- Streaming service plumbing confirmed: `iteration` starts at `1u32`
  (streaming.rs:193), is passed into the per-loop `StreamContext`
  (streaming.rs:335/362), and advances with `iteration += 1` (streaming.rs:639).
  The iter-1-placeholder-filter vs iter-2+-keep behavior is documented in the
  service (streaming.rs:206–207).
- `git blame` confirms the offending `else if` push originated in `da0e77e6`
  (round-7), making this a genuine regression.

Minor inaccuracies in the original finding (the file is
`modules/chat/core/services/streaming.rs` not a bare `streaming.rs`; a few line
numbers were off; the trailing kept message is `Role::Tool` not `Assistant`)
do not change the verdict.

### Corrected fix

Scope the current-upload inline to the iteration the upload actually belongs to
(iteration 1):

1. Preferred — wrap the entire current-upload inline block
   (`if let Some(file_ids) = &send_request.file_ids { … }`, file.rs:140–204)
   in `if context.iteration == 1 { … }`. On iteration ≥2 the current upload is
   already covered by the manifest + replay, so re-inlining it (appended or
   pushed) is never wanted; this also avoids the redundant per-iteration
   `process_file_blocks` DB work.

2. Minimum — change the else-if guard at file.rs:197 from
   `} else if !file_blocks.is_empty() {` to
   `} else if context.iteration == 1 && !file_blocks.is_empty() {`.

Add a Tier-6 / integration test driving a tool-calling loop with a non-image
attachment on a tool-capable model: assert the attachment content is inlined
exactly once (iteration 1) and that **no** trailing `Role::User` message is
appended after the assistant tool turn on iteration ≥2 (assert
`request.messages` ends with `Role::Tool` / the assistant continuation, not a
re-inlined `User` turn).

Keep the hand-narrow ~80-col Rust style; do not `rustfmt`.
