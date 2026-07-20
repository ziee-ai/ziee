# PLAN_AUDIT — fix conversation titles under `manual_approve`

Audited against the codebase at `4b2a6d898` (branch point off `khoi`). Diff base for every gate is
`origin/khoi`, **not** `origin/main` — this branch is cut from `khoi`, so the default base would drag
PR #165's 7 files into the phase-6 coverage law. All gate invocations pass `--base origin/khoi`.

## Breakage risk

**ITEM-1 (new trait method).** `ChatExtension` has **19** `impl` blocks (18 production + the
`ProbeExtension` test double at `registry.rs:662`). A new method **with a default impl** is
source-compatible with all of them — none needs editing, and no extension changes behavior. The trait is
crate-local (`modules/chat/core/extension/registry.rs`); `ExtensionRegistry` is the generic container
from `ziee-framework`, which is parameterized over `dyn ChatExtension` and does not enumerate methods,
so the SDK submodule needs no change. `ziee-desktop` depends on `ziee` but does not implement the trait.

**ITEM-3 (streaming call sites) — the highest-risk item in the plan.** Three specific hazards:

1. *SSE ordering.* `start_generation` (`streaming.rs:792-985`) drains `ext_rx` non-blockingly at its
   tail (`:953-964`) on the documented invariant that extension events are emitted **before** the
   terminal chunk. Calling the hook after the terminal chunk would drop the `TitleUpdated` event on the
   floor. Mitigation is in the plan (hook precedes the terminal chunk) and is asserted by the e2e/
   integration test, which reads the title from the DB *and* checks the event.
2. *Double-fire.* If the hook were ever also wired into the normal path, title would run twice per turn.
   `title_test.rs::title_call_count` (`title_test.rs:66-68`) is the existing tripwire and must stay
   green; TESTS.md adds an explicit assertion rather than relying on it incidentally.
3. *Blocking the terminal chunk.* The hook is `await`ed inline, so a slow title provider now delays the
   `stop` chunk on the skipped path exactly as it already does on the normal path (out-of-scope item 3
   covers making this async). Same latency profile as today — not a regression, but recorded.

**ITEM-10 (list-query change).** `get_conversations` (`repository/conversations.rs:158-199`) is the
single list query and already `GROUP BY c.id` with a `LEFT JOIN` fan-out to `branch_messages`. A naive
correlated subquery in the SELECT list would have to be added to the GROUP BY or wrapped; a
`LEFT JOIN LATERAL (… LIMIT 1) ON TRUE` avoids both and cannot multiply rows. Risk: the `ORDER BY`
already references `COUNT(bm.message_id)`, so the lateral must not disturb the aggregate — it joins
one row per conversation, so it does not. `ConversationResponse` uses `#[serde(flatten)]` over
`Conversation` (`types/conversation.rs:49-55`); the new field goes on `ConversationResponse` (the
projection), **not** on `Conversation` (the DB row model), so no other consumer of `Conversation`
changes. Adding an `Option<String>` field is additive on the wire — existing clients ignore it.

**ITEM-7/8 (frontend).** `src-app/desktop/ui` does **not** duplicate these components
(`grep 'Untitled Conversation' src-app/desktop/ui/src` → zero hits; its `modules/chat/` contains only
`core/`), so the change is confined to `src-app/ui`. Desktop is still affected by the regenerated
`types.ts`, which is mechanical. `PaneManagerDrawer.tsx:75` uses a *different* fallback
(`|| 'Conversation'`) and was missed by the original 8-site survey — folding it into the shared helper
is correct but is a 9th edit, recorded so it is not mistaken for scope drift at phase 6.

**ITEM-6 (loop terminator).** Highest behavioral-risk item after ITEM-3: terminating a turn early on a
repeated unroutable name changes agentic-loop behavior for ALL models, not just gpt-oss. It must key on
*unroutable-and-already-errored-this-turn*, never on a legitimately repeated successful tool call
(re-calling the same tool with different arguments is normal and must keep working).

## Pattern conformance

- **ITEM-1/2** mirror the existing `after_llm_call` pair exactly — trait method with default impl
  (`registry.rs:142-149`) + registry fan-out (`registry.rs:401-423`), same `#[async_trait]`, same
  `Option<&UnboundedSender<…>>` tx param, same `self.inner.iter()` (pre-sorted by `order`). The ONE
  deliberate deviation is error handling: `call_after_llm_call` uses `?` (one extension's error aborts
  the fan-out); the new fan-out swallows-and-logs, because on this path the user's answer is already
  persisted and streamed. That deviation is documented inline at the definition, per the repo rule that
  a divergence from the mirrored pattern must carry its rationale.
- **ITEM-2** reuses the title extension's existing self-gating (`has_title` / `should_generate_title`,
  `title.rs:118-142`) rather than adding a second guard. The extracted routine takes `(&StreamContext,
  tx)` — provably sufficient, since `after_llm_call` already ignores `_final_message` (`title.rs:375`).
- **ITEM-10** follows the active-branch scoping precedent set by the search `EXISTS` subquery in the
  SAME query (`conversations.rs:171-181`), including its documented rationale (superseded edit-branch
  content is invisible when the conversation is opened). Using a different branch scope for the preview
  than for search would be an inconsistency.
- **Regression test** mirrors `tests/chat/title_audience_test.rs` (`oai_capture_stub` + `MockMcpServer` +
  `audience:["user"]`), flipping the `approval_mode` it explicitly opts out of at `:135-141`. The
  approve-and-resume shape copies `tests/mcp/mcp_approval_loop_test.rs:144-256`; the pending row is read
  via `GET /branches/{branch_id}/pending-approvals` as `mcp_approval_workflow_test.rs:560-621` does —
  NOT the repro script's direct-Postgres query, which is fixture-coupled.
- **ITEM-7** introduces the first shared `conversationDisplayLabel` helper; today the fallback is
  copy-pasted at 9 sites with 2 different literals. Consolidating is the conformant move.

## Migration collisions

**None — this work adds no migration.** Migrations are per-module
(`src-app/server/src/modules/*/migrations/`), and ITEM-10 is a *projection* change (a LATERAL subquery
plus a response field), not a schema change: no new column, table, index, or permission. `sqlx`
compile-time verification will re-check the modified query against the per-worktree build DB
(`ziee_build_<key>`) on the next `cargo check`; no `cargo clean` is needed because no migration file
changes.

## OpenAPI regen

**Required, and it is the one cross-cutting mechanical step.** ITEM-10 adds a field to
`ConversationResponse`, a `schemars`-derived type in the spec. `just openapi-regen` must run for **BOTH**
binaries — server → `src-app/ui/{openapi/openapi.json, src/api-client/types.ts}` and desktop →
`src-app/desktop/ui/{…}`. Four generated files.

Two consequences to respect:
- `openapi::emit_ts::tests::types_ts_parity` is a byte-for-byte golden test — a field added without
  regenerating **fails the test suite**. Regen is not optional cleanup.
- The phase-6 coverage law and the phase-3/8 frontend gates **exclude** `**/openapi.json` and
  `**/api-client/types.ts`, so the regen does not by itself make this a UI diff and its hunks need no
  audit angles. The real frontend work (ITEM-7/8) does make it a UI diff, so a `tier: e2e` test is
  still mandatory at phase 3.

No other item touches a serialized type: the new trait hook, the streaming call sites, and the MCP
diagnostics are all internal.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors the `after_llm_call` trait+registry pair; default impl keeps all 19 existing `impl ChatExtension` blocks source-compatible; no SDK change
- **ITEM-2** — verdict: PASS — title's hook deps are `(context, tx)` only, proven by `_final_message` being unused at `title.rs:375`; reuses existing self-gating
- **ITEM-3** — verdict: CONCERN — touches the streaming loop; must precede the terminal chunk (SSE drain invariant `streaming.rs:953-964`) and must NOT be wired into the normal path (double-fire). Both are asserted by enumerated tests, not left to review
- **ITEM-4** — verdict: PASS — diagnostics only; log-level change at `mcp.rs:3672-3676`, no behavior change
- **ITEM-5** — verdict: CONCERN — deliberately unspecified pending live diagnosis; carries the approved split gate (own PR if the cause is large or model-side). Tracked as a DECISION so it cannot become a silent scope drop
- **ITEM-6** — verdict: CONCERN — changes agentic-loop termination for ALL models; must key strictly on *unroutable AND already-errored-this-turn* so a legitimately repeated tool call is unaffected
- **ITEM-7** — verdict: PASS — confined to `src-app/ui` (desktop does not duplicate these components); depends on ITEM-10
- **ITEM-8** — verdict: PASS — filters the already-loaded list against the same helper the row renders; server-side content search already exists independently (`conversations.rs:171-181`) and is untouched
- **ITEM-9** — verdict: PASS — three trivial test-only edits, no production code
- **ITEM-10** — verdict: CONCERN — requires `just openapi-regen` for BOTH ui and desktop or the `types_ts_parity` golden test fails; LATERAL join must not disturb the existing `COUNT(bm.message_id)` aggregate or the `GROUP BY c.id`
