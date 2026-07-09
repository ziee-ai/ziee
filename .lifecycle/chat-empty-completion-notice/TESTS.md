# TESTS — chat-empty-completion-notice

Bipartite mapping: every ITEM covered by ≥1 TEST; the UI diff carries a `tier: e2e` test.

## Tests
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: `is_visible_answer` returns false for `("thinking", "x")`, `("text", "")`, `("text", "   ")`; true for `("text", "hi")`, `("tool_use", "")`, `("tool_result", "")`, `("image", "")`, `("file_attachment", "")`.
- **TEST-2** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/chat/stub_chat_tier2_test.rs` — asserts: with a `StubChat` reply that has `reasoning_content` set but empty `content` and no `tool_calls`, the collected terminal `complete` SSE frame carries `finish_reason == "empty"` and NO `error` frame is emitted; a control reply with normal text yields `finish_reason == "stop"` (unchanged). (Appended to the existing stub-chat tier-2 file to reuse its `run_turn`/`create_model` harness — see DRIFT-1.)
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/chat/components/emptyCompletion.test.ts` — asserts: the `hasVisibleAnswer`/`isVisibleAnswerBlock` predicates are false for an assistant message whose only block is `thinking` (and for a zero-content message), and true when a `text`(non-empty)/`tool_use`/`image` block is present.
- **TEST-4** (tier: e2e) [covers: ITEM-4, ITEM-5, ITEM-6] file: `src-app/ui/tests/e2e/chat/empty-completion.spec.ts` — asserts: driving a conversation whose assistant turn returns an empty completion, the `chat-empty-completion-notice` alert is visible inside the assistant message, and it REMAINS visible after a full page reload (reload-robust render-time detection).

## Notes
- TEST-1/TEST-3 are pure (no DB / no network). TEST-2 needs Postgres + the in-process
  `StubChat` (no API keys). TEST-4 needs Playwright + a mocked/stub provider returning empty.
- ITEM-5 (zero-content guard restructure) and ITEM-6 (testid registry) are exercised by
  TEST-4 (the notice must render for a finalized empty assistant message and be selectable by
  its registered testid); ITEM-6 is additionally guarded at build time by
  `check:testid-registry` inside `npm run check` (Phase 8).
