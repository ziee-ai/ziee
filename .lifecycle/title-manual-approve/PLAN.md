# PLAN — fix conversation titles under `manual_approve`

Branch `fix/title-manual-approve` off `khoi`. PR **into `khoi`** (platform-wide bug fix), do not merge.

## Problem

PR #165 fixed auto-titling for the `auto_approve` path. Production runs `manual_approve` on **all 11
conversations**, and under that mode a conversation stays "Untitled Conversation" through the ENTIRE
first exchange (approval → tool run → answer all complete) and is only titled when the user sends a
**second** message. The originally-reported symptom is therefore still visible on the deployment's
default configuration.

**Verified root cause.** `call_after_llm_call` — where the title extension (order 80) runs — has
exactly ONE call site: `DeltaAccumulator::finalize()` (`streaming.rs:1581`), which only runs after a
provider stream is consumed. Two arms of the streaming loop end the turn without ever calling the
provider, so `finalize()` is never constructed and no `after_llm_call` fires:

- `streaming.rs:402-416` — `BeforeLlmAction::Complete`: sends `extension_complete`, `break`s.
- `streaming.rs:418-453` — `BeforeLlmAction::CompleteWithContent { text }`: appends the text via
  `append_content`, streams it, sends `stop`, `break`s.

There is no post-loop code at all (`streaming.rs:769-774` is a comment block), so a `break` ends the
spawned task outright. MCP (order 30) produces those actions on the approval-resume send at
`mcp.rs:1591` (all denied), `:1613` (approvals still pending), `:1671` (approved tool returned
`audience:["user"]` content). The third is the production path. The PR #165 registry fix cannot help —
the hook is never reached at all.

## Items

- **ITEM-1**: Add an `after_llm_skipped` hook to the `ChatExtension` trait (default no-op) plus
  `ExtensionRegistry::call_after_llm_skipped`, which runs every extension in order and logs-but-swallows
  per-extension errors (unlike `call_after_llm_call`, whose `?` aborts the fan-out) so a turn that has
  already produced its answer is never failed by a title-generation error.
- **ITEM-2**: Implement `after_llm_skipped` on the title extension, delegating to the SAME private
  routine `after_llm_call` already uses (extract it if needed) so the two hooks cannot drift apart.
- **ITEM-3**: Call `call_after_llm_skipped` from the two `break` arms in `streaming.rs`, BEFORE the
  terminal chunk (so the `TitleUpdated` SSE event precedes it — `start_generation` drains `ext_rx` on
  that invariant, `streaming.rs:953-964`). In the `CompleteWithContent` arm it must run AFTER the text
  `append_content` so `assistant_produced_output` (`title.rs:91-99`) sees the answer.
- **ITEM-4**: Diagnose the gpt-oss unprefixed-tool-name failure live. The brief's suggested direction
  is ALREADY implemented (`resolve_server_and_tool`, `mcp.rs:354-373`, does unique-suffix recovery;
  unroutable tool_uses already get synthetic error tool_results at `mcp.rs:2894-2918`/`:2922-2943`/
  `:930-952`; `max_iteration` defaults to 10, not unlimited, `defaults/models.rs:62`). Add diagnostics
  at the warn site (`mcp.rs:3672-3676`) logging the unresolved name ALONGSIDE the bare-name map's keys
  and ambiguity state, so one repro run distinguishes the three hypotheses (H1 `query_rag` advertised
  by ≥2 servers → marked ambiguous; H2 map never built because `before_llm_call` returned `Continue`
  at `mcp.rs:1820`/`:1878` before the map block at `:2174`; H3 model emits a name matching no
  advertised bare name).
- **ITEM-5**: Fix the cause ITEM-4 identifies. **Split gate:** if the diagnosis reveals a large cause,
  or one that is model-side rather than a ziee bug, ITEM-5 moves to its own PR and this PR ships
  ITEMs 1-3 + 6-9.
- **ITEM-6**: Harden the loop independent of cause — detect a `tool_use` that is unroutable AND repeats
  a name already answered with a routing error this turn, and terminate with `ExtensionAction::Complete`
  instead of feeding the model another identical error and burning iterations. Kills the
  "spins and never terminates" class even if the root cause is model-side.
- **ITEM-7**: Frontend — when `title` is null, render `first_message_preview` (ITEM-10) as the display
  label; leave the DB `title` column null so a real title can still land later. ONE shared helper
  (`conversationDisplayLabel`) used at every render site rather than repeating the expression — no such
  helper exists today, the fallback is copy-pasted at 9 sites. Keep "Untitled Conversation" as the final
  fallback when the preview is absent too. `TitleEditor` must keep editing/saving the real `title`
  field — the label is display-only and must NEVER be written back on blur.
- **ITEM-8**: Frontend — fix the two client-side search filters that match the placeholder literal
  (`ConversationPickerPane.tsx:47`, `PaneManagerDrawer.tsx:121`) so an untitled conversation is findable
  by its content instead of only by typing "Untitled". They filter the already-loaded list, so they match
  against the same `conversationDisplayLabel` the row renders.
- **ITEM-10**: Backend — add `first_message_preview: Option<String>` to `ConversationResponse`
  (`chat/core/types/conversation.rs:49-63`), populated by a LATERAL subquery over the ACTIVE branch's
  first `text` message content in the existing list query (`chat/core/repository/conversations.rs:158`),
  truncated server-side. **Discovered in phase 2, approved by the lead:** the sidebar renders from
  `ChatHistory.conversations` (`ConversationResponse[]`) and no message text crosses the wire for the
  list endpoint, so ITEM-7 is impossible without this — the per-conversation message cache only holds
  conversations opened this session, leaving cold rows untitled. Requires `just openapi-regen` for BOTH
  `ui` and `desktop/ui`. Scope the subquery to `c.active_branch_id`, matching the existing search
  `EXISTS` subquery's documented rationale (superseded edit-branch content is invisible when the
  conversation is opened).
- **ITEM-9**: Trivial test-debt cleanups enabled by PR #165's seam: vacuous `.last()` assertion
  (`tests/bio_mcp/mod.rs:465`) → `.find(|r| !r.is_title_request)`; hardcoded title literal
  (`tests/file/provider_routing_integration_test.rs:134`) → `common::stub_chat::TITLE_PROMPT_PREFIX`;
  wrong module doc (`tests/chat/stub_chat_tier2_test.rs:5`).

### Deliberate non-goals

The error breaks (`streaming.rs:354`, `:508`) and the streaming failsafe `max_iterations` break
(`:233`) are left alone: a failed turn should not be titled, and the failsafe path already ran
`finalize()` on its prior iteration. Out-of-scope items 3 (async title generation) and 5 (provider
finish-reason fidelity) stay deferred per the brief. Item 6 (101 pre-existing test failures on `khoi`)
is explicitly out of scope.

## Files to touch

- `src-app/server/src/modules/chat/core/extension/registry.rs` — trait method + `call_after_llm_skipped`
- `src-app/server/src/modules/chat/core/services/streaming.rs` — the two `break` arms
- `src-app/server/src/modules/chat/extensions/title/title.rs` — implement the hook via a shared routine
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — ITEM-4 diagnostics, ITEM-5 fix, ITEM-6 terminator
- `src-app/server/tests/chat/title_test.rs` (or a new `title_approval_test.rs`) — the headline regression
- `src-app/server/src/modules/chat/core/types/conversation.rs` — `first_message_preview` field (ITEM-10)
- `src-app/server/src/modules/chat/core/repository/conversations.rs` — LATERAL subquery (ITEM-10)
- `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`,
  `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts` —
  mechanically regenerated by `just openapi-regen` (excluded from the audit coverage law)
- `src-app/ui/src/modules/chat/` — the shared `conversationDisplayLabel` helper + render sites +
  2 search filters: `components/TitleEditor.tsx:154`, `components/ConversationCard.tsx:115`+`:147`,
  `widgets/RecentConversationsWidget.tsx:250`+`:374`, `components/ConversationPickerPane.tsx:47`+`:152`,
  `components/PaneManagerDrawer.tsx:121`+`:277` (and `:75`, which uses a DIFFERENT `'Conversation'`
  fallback — a 9th site the original survey missed)
- `src-app/server/tests/bio_mcp/mod.rs`, `tests/file/provider_routing_integration_test.rs`,
  `tests/chat/stub_chat_tier2_test.rs` — ITEM-9

## Patterns to follow

- **New trait hook + registry fan-out** — mirror the EXISTING `after_llm_call` pair in
  `chat/core/extension/registry.rs:142-149` (trait method with a default impl) and `:401-423`
  (`call_after_llm_call`). Same `#[async_trait]` shape, same `Option<&UnboundedSender<…>>` tx param,
  same iteration over `self.inner.iter()` (already sorted by `order`). Deviate ONLY in error handling
  (swallow-and-log rather than `?`), and document WHY inline.
- **Title extension** — it already re-reads conversation + history from the DB and ignores
  `_final_message` (`title.rs:375`), so the shared routine takes just `(&StreamContext, tx)`. Keep the
  existing `has_title` / `should_generate_title` self-gating; do not add a second guard.
- **Regression test** — mirror `tests/chat/title_audience_test.rs` (which already wires
  `oai_capture_stub::StubChat` + `MockMcpServer` + an `audience:["user"]` tool), changing its
  `approval_mode` from the `auto_approve` it explicitly opts into at `:135-141` to `manual_approve`,
  and resuming with `tool_approvals`. Read the pending row via
  `GET /branches/{branch_id}/pending-approvals` (as `tests/mcp/mcp_approval_workflow_test.rs:560-621`
  does), NOT the repro script's direct-Postgres query.
- **Approval resume shape** — there is no separate approve endpoint; the decision rides a fresh
  `POST /conversations/{id}/messages` with a `tool_approvals` array. Copy
  `tests/mcp/mcp_approval_loop_test.rs:144-256`.
- **Frontend display label** — a single exported helper in the chat module, imported by all six sites.
  No new component: every call site already renders the string it gets. Follow the existing
  module-local helper placement in `src-app/ui/src/modules/chat/`.

## UI-surface checklist (ITEM-7 / ITEM-8)

This adds **no new surface** — it changes the string six existing surfaces already render, so most of
the UI-surface checklist is not applicable by construction. The parts that are:

- **Precedent** — none needed; each call site keeps its current typography, tokens, and container.
  The only change is the value of an already-rendered string.
- **Scale / cardinality** — unchanged, and explicitly NO N+1. The preview rides the EXISTING paginated
  list response (ITEM-10) as one extra column, so the sidebar issues exactly the same number of requests
  as today. The per-row cost is one LATERAL subquery over an indexed `branch_messages`/`message_contents`
  lookup, bounded by the existing `LIMIT/OFFSET`. Truncate server-side so a long first message cannot
  bloat the list payload.
- **Responsive** — the derived label can be LONGER than "Untitled Conversation", so every call site
  must keep its existing truncation/ellipsis behavior at ~390px. Verified in the e2e at narrow
  viewport; a label that overflows its row is a defect.
- **Populated render** — the states that matter are: titled (unchanged), untitled-with-first-message
  (new label), and untitled-with-no-message (falls back to "Untitled Conversation"). All three need a
  gallery/e2e render, not just the happy path.
- **JTBD** — the job is "find the conversation I was just in, in a sidebar where the model failed to
  title anything." Today every such row is the identical string and the search box only matches the
  word "Untitled", so the job is impossible; after the change the row shows what the user actually
  asked and search matches it.
