# TESTS — fix conversation titles under `manual_approve`

Bipartite mapping: every ITEM has ≥1 covering TEST; every TEST names a valid ITEM, tier, target file,
and assertion. Tiers mirror the repo's existing structure — unit `#[cfg(test)]`, integration
`src-app/server/tests/<module>/`, e2e `src-app/ui/tests/e2e/`.

The diff touches `src-app/ui/src/modules/chat/**` (ITEM-7/8), so a `tier: e2e` spec is mandatory and is
enumerated below (TEST-12, TEST-13). No new permission is introduced by any item — no
`[negative-perm]` spec is required (the A10 gate does not apply; `first_message_preview` rides the
already-gated `GET /conversations`, which enforces existing conversation ownership).

## The headline regression

- **TEST-1** (tier: integration) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/server/tests/chat/title_approval_test.rs` — asserts: under `approval_mode: "manual_approve"`, a first turn whose approved `audience:["user"]` tool result IS the answer gets the conversation TITLED on that resume — no second user message. Drives the real path: turn 1 → assert a pending approval exists AND `title IS NULL`; resume via `POST /messages` with `tool_approvals:[{tool_use_id, decision:"approved"}]` (pending row read from `GET /branches/{branch_id}/pending-approvals`); assert `title == STUB_TITLE` immediately after that resume completes. This is the test that would have caught the reported bug.

## ITEM-1 — the registry fan-out

- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/chat/core/extension/registry.rs` — asserts: `call_after_llm_skipped` invokes EVERY registered extension in `order` sequence (using the existing `ProbeExtension` double at `registry.rs:662`), recording call order — proving it does not short-circuit the way `call_before_llm_call` does.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/chat/core/extension/registry.rs` — asserts: when one extension's `after_llm_skipped` returns `Err`, the fan-out still runs the REMAINING extensions and the call itself returns `Ok` — the swallow-and-log contract that keeps an already-answered turn from failing on a title error. (Contrast: `call_after_llm_call` aborts on `?`.)
- **TEST-4** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/chat/core/extension/registry.rs` — asserts: the trait's DEFAULT `after_llm_skipped` impl is a no-op returning `Ok(())`, so the 18 extensions that do not implement it are unaffected.

## ITEM-2 — the title extension hook

- **TEST-5** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/chat/extensions/title/title.rs` — asserts: the existing pure-function gating tests (`has_title`, `should_generate_title`, `assistant_produced_output`, `clean_generated_title`) remain green after the routine is extracted, proving the extraction changed no gating behavior. **Honest scope:** that both hooks reach the same body is a STRUCTURAL property (each is a one-line delegation to `title_if_needed`), verified by reading, not by this test; TEST-1 is what proves the skipped path actually titles.

## ITEM-3 — the streaming call sites

- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/chat/title_approval_test.rs` — asserts (by documented NON-assertion): the strict `titleUpdated`-before-terminal-frame ordering is deliberately NOT asserted. **Finding from the audit round:** the driver forwards the chunk stream and the extension channel through ONE `tokio::select!`, which picks arbitrarily among ready branches, so the two frames publish in either order — asserting it produced a flaky test (observed failing under `--test-threads=4` while the title was correctly persisted). The ordering is not required in production: the client's per-conversation SSE connection is long-lived and keeps receiving after `complete`, and the turn also publishes `Conversation/Update` for other surfaces. The deterministic delivery guarantee — the title persisted on turn 1 — is TEST-1.
- **TEST-7** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/chat/title_approval_test.rs` — asserts: a DENIED approval (`decision:"denied"`, the `mcp.rs:1591` all-denied → `BeforeLlmAction::Complete` path) generates NO title and completes cleanly — the new hook is a safe no-op when the turn produced no answer.
- **TEST-8** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/chat/title_test.rs` — asserts: the normal (non-skipped) path is unchanged — `normal_model_gets_an_ai_generated_title_on_the_first_exchange` keeps `title_call_count == 1` across TWO turns, so the streaming edit did not perturb the path `after_llm_call` already covered. **Honest scope:** this does NOT detect the hook being additionally wired into the normal path — a second invocation would short-circuit on `has_title` without an LLM call, leaving the count at 1. That case is prevented by construction (the hook is called ONLY inside the two `BeforeLlmAction` break arms, which `break` before any provider call) and is visible in the diff; it is a wasted-work risk, not a correctness one.

## ITEM-4 / ITEM-5 / ITEM-6 — gpt-oss tool-name routing

- **TEST-9** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `describe_advertised_tools` renders each advertised tool with its ambiguity state (`name=<uuid>` vs `name=<ambiguous>`), and `<none advertised this turn>` for an empty map — so one repro run distinguishes H1 (ambiguous) from H2 (map empty) from H3 (name absent). Output is sorted so two reports can be diffed. Asserts on the rendered payload, not on log plumbing. **Implemented and passing** (`advertised_tools_diagnostic_distinguishes_the_three_causes`).
- **TEST-10** [DESCOPED with ITEM-5] (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: the resolution behavior for whichever cause the live repro identifies, alongside the existing `resolve_server_and_tool` / `recover_server_id_for_bare_name` cases. **Written only after the diagnosis**; if the split gate fires (cause is large or model-side), ITEM-5 and this test move to the follow-up PR and are recorded as a DESCOPE in DECISIONS.md.
- **TEST-11** [DESCOPED with ITEM-6] (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: a `tool_use` that is unroutable AND repeats a name already answered with a routing error this turn terminates with `ExtensionAction::Complete`; AND the negative control — a legitimately repeated tool call that RESOLVES (same name, different arguments) still returns `Continue` and is not caught by the terminator.

> **TEST-10 / TEST-11 are DESCOPED** along with ITEM-5 / ITEM-6 — see DECISIONS DEC-20. The live
> repro on the review container showed gpt-oss's prefix-less `query_rag` ALREADY resolves correctly
> (`[mcp] Recovered server_id for prefix-less tool name 'query_rag' -> …`, zero warnings), so there is
> no defect for them to cover. ITEM-4 (TEST-9) still ships.

## ITEM-7 / ITEM-8 / ITEM-10 — untitled display label

- **TEST-12** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/untitled-conversation-label.spec.ts` — asserts: a user with an untitled conversation sees the FIRST USER MESSAGE text as the sidebar row label after a FULL PAGE RELOAD (the cold-cache case that motivated the backend field — the client message cache is empty here); a conversation WITH a title still shows its title; and a conversation with neither shows "Untitled Conversation". All three states, at desktop AND ~390px viewport, asserting no row overflow.
- **TEST-13** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/untitled-conversation-label.spec.ts` — asserts: typing text from an untitled conversation's first message into the picker/drawer search box FINDS that conversation (today it is findable only by typing "Untitled"), and that typing "Untitled" no longer matches a conversation that now renders a real preview.
- **TEST-14** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/chat/core/utils/conversationDisplayLabel.test.ts` — asserts: the helper's precedence — non-empty `title` wins; else non-empty `first_message_preview`; else the literal "Untitled Conversation"; whitespace-only `title` is treated as absent (matching the backend's `has_title` semantics at `title.rs:118-120`).
- **TEST-15** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/untitled-conversation-label.spec.ts` — asserts: `TitleEditor` still edits and SAVES the real `title` field — opening the editor on an untitled conversation does NOT prefill or persist the derived preview as the title (the display-only contract; a regression here would silently write the raw first message as a permanent title, re-introducing exactly what PR #165 removed).
- **TEST-16** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/chat/conversations_test.rs` — asserts: `GET /conversations` returns `first_message_preview` populated from the ACTIVE branch's first text message, `null` when the conversation has no user text, truncated to the documented cap, and that the row's `message_count` and ordering are UNCHANGED by the LATERAL join (the aggregate-disturbance risk from PLAN_AUDIT).
- **TEST-17** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: `types_ts_parity` (the existing byte-for-byte golden test) passes, proving `just openapi-regen` ran for both binaries after the response-type change.

## ITEM-9 — test-debt cleanups

- **TEST-18** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/bio_mcp/mod.rs` — asserts: the de-vacuated assertion actually discriminates — `.find(|r| !r.is_title_request)` inspects the real chat request rather than `.last()` (which was already the title request, so the old assertion could not fail). Must still pass after the change.
- **TEST-19** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/file/provider_routing_integration_test.rs` — asserts: the title-prompt check uses `common::stub_chat::TITLE_PROMPT_PREFIX` rather than a hardcoded literal, so a prompt reword cannot silently un-cover this test.

## Regression surface that must stay green

`tests/chat/title_test.rs` (5 tests) and `tests/chat/title_audience_test.rs` (1 test) run unchanged
except for TEST-8's added assertion. `tests/mcp/mcp_approval_workflow_test.rs` and
`mcp_approval_loop_test.rs` exercise the approval paths the streaming change touches and must not
regress. Baseline for the full suite is the known **101 pre-existing failures** on `khoi` (72 missing
`tests/.env.test`, 14 sandbox rootfs, 8 pagination-shape, ~7 npx flakes) — compare against that, do not
chase them.
