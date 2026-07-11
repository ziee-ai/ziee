# PLAN_AUDIT — tool-artifact-grouping (follow-up #3)

Audit of PLAN.md against the codebase at base `origin/khoi` @ `ab6127e1`.

## Breakage risk

- **New McpComposer subscription in ConversationPage (ITEM-1).** ConversationPage
  doesn't read `McpComposer` today; adding `const { toolCalls } = Stores.McpComposer`
  is a new reactive dependency → ConversationPage re-renders on any tool-status change.
  ConversationPage already re-renders on `messages`/streaming, so the extra re-renders
  are marginal, and the effect only calls `scrollToBottom` for a genuinely-new
  `pending_approval` id (deduped). Low risk.
- **Unconditional scrollToBottom (ITEM-1).** Deliberately bypasses `isAtBottom` — that's
  the fix. It could yank a user who scrolled up when an approval appears, but that IS the
  desired behavior (the approval needs their attention). Deduped per id so it fires once,
  not on every stream delta. The conversation-match guard prevents firing during the
  stale A→B switch window.
- **Timing: is the approval row measured when scrollToBottom fires?** The
  `mcpApprovalRequired` handler updates BOTH `toolCalls` and appends the tool_use block to
  `streamingMessage.contents` in one handler; React commits both, then the effect
  (`[toolCalls]`) runs → the virtualizer count already includes the row.
  `scrollToBottom` = `virt.scrollToIndex(count-1,{align:'end'})` + `startReassert`
  (settles on measured heights), so a just-added row is handled. If a one-frame lag
  appears in practice, wrap in `requestAnimationFrame` (noted).
- **Removing the dead scroll (ITEM-2)** is pure deletion of #134 code that does nothing;
  no behavior depends on it. Zero risk.
- **E2E determinism (ITEM-3).** The reproduction relies on `isAtBottom` staying false
  after scrolling to top through the send (send doesn't force-scroll — verified). The
  `chat-jump-to-latest-btn` visibility is the deterministic proxy for `isAtBottom===false`.
  May need iteration to make the overflow + reload timing robust (phase 5/8).

## Pattern conformance

- Reactive read + effect mirror ConversationPage's own auto-follow effect and the
  `McpToolUseRenderer` subscription pattern. Conformant.
- `scrollToBottom()` is the established handle method already used at
  `ConversationPage.tsx:272`. Conformant.
- E2E mirrors `jump-to-latest.spec.ts` + `tool-group-auto-open.spec.ts`. Conformant.

## Migration collisions

- **None.** No migration/DB/backend. N/A.

## OpenAPI regen

- **None.** No backend type/route change. `check:state-matrix` regen of the generated
  gallery files may be needed (mechanically generated; not OpenAPI).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — reuses `scrollToBottom` + the reactive-read pattern; drops only the `isAtBottom` gate (the bug); deduped + conversation-gated. No caller breakage.
- **ITEM-2** — verdict: PASS — deletes dead #134 code; nothing depends on it.
- **ITEM-3** — verdict: CONCERN — the below-the-fold reproduction needs careful, deterministic setup (overflow + not-at-bottom + tail approval); not a blocker, iterated + verified in phase 8. It replaces a false-green spy test with an effect assertion.
