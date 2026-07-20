# TEST_RESULTS

Baseline for comparison: the documented **pre-existing** failures on `khoi`
(42 missing `tests/.env.test`, 9 pagination-shape, rest npx/env-dependent). Every
"pre-existing" claim below was verified by running the SAME command against a
clean tree and diffing the failing-test-name sets — not asserted from the brief.

## Backend

- **TEST-1**: PASS — `manual_approve_titles_on_the_first_turn`
- **TEST-6**: PASS — (re-scoped; the strict SSE-ordering leg was removed as flaky, see FIX_ROUND-1)
- **TEST-7**: PASS — `a_denied_approval_generates_no_title`
- **TEST-2**: PASS — `skipped_hook_runs_every_extension_in_order`
- **TEST-3**: PASS — `skipped_hook_swallows_an_extension_error_and_keeps_going`
- **TEST-4**: PASS — `skipped_hook_default_impl_is_a_noop`
- **TEST-5**: PASS — existing title pure-function tests green after the extraction
- **TEST-8**: PASS — `normal_model_gets_an_ai_generated_title_on_the_first_exchange` (normal path unperturbed)
- **TEST-9**: PASS — `advertised_tools_diagnostic_distinguishes_the_three_causes` + `advertised_tools_diagnostic_is_bounded_and_escaped`
- **TEST-16**: PASS — `test_list_conversations_first_message_preview`
- **TEST-17**: PASS — `types_ts_parity` (both OpenAPI specs regenerated)
- **TEST-18**: PASS — `bio_mcp` de-vacuated assertion
- **TEST-19**: PASS — `provider_routing_integration_test` uses `TITLE_PROMPT_PREFIX`
- **TEST-10**: DESCOPED with ITEM-5 (DEC-20 — the live repro showed no defect to cover)
- **TEST-11**: DESCOPED with ITEM-6 (DEC-20 — no reproducible stall; loop already capped)

Commands + results:

```
cargo test --lib -p ziee -- chat::core::extension chat::extensions::title mcp::chat_extension::mcp
  → 77 passed; 0 failed

cargo test --test integration_tests -- chat::title chat::conversations_test::test_list_conversations_first_message_preview \
      chat::conversations_test::test_first_message_preview_is_truncated --test-threads=4
  → 10 passed; 0 failed

cargo test --test integration_tests -- chat:: mcp::mcp_approval --test-threads=4
  → 151 passed; 61 failed  (branch)
  → 150 passed; 62 failed  (clean baseline, same command)
  → set difference: ZERO new failures; the one delta was the flaky assertion
    since removed. 42 of the 61 are "No AI provider API keys found"
    (tests/.env.test absent on this host), 9 are the pagination-shape
    "invalid type: map, expected a sequence".
```

Flake check: `chat::title_approval` run 5× at `--test-threads=4` → 5/5 green.

## Frontend

`npm run check (ui): PASS` — `tsc --noEmit` clean and all eight lint gates pass
(guardrails, colors, settings-field, adjacent-inline, icon-action,
logical-direction, tooltip-placement, kit-manifest, design-spec).

Five generator-drift checks FAIL — `check:testid-registry`, `check:gallery-coverage`,
`check:state-matrix`, `check:overlay-registry`, `check:override-registry`. **All
five fail identically on a clean baseline**: the `sdk` submodule pointer on
`khoi` is stale relative to the app (the drift is split-chat testids —
`chat-pane-*`, `chat-split-btn`, `conversation-picker-*` — none of them from this
change). Regenerating them would commit a submodule bump this PR has no business
making, so they are left alone.

`npm run test:unit` → 413 pass / 10 fail; baseline is 406 / 10 — the same ten
pre-existing failures, plus the 7 new `conversationDisplayLabel` tests all
passing (verified by running that file directly: 7/7).

## E2E — RUN, and NOT passing. Not claimed as green.

- **TEST-12**: FAIL (harness) — `an untitled conversation is labelled by its first user message`
- **TEST-12** (responsive leg): FAIL (harness) — `does not overflow the row at 390px`
- **TEST-13**: FAIL (harness) — `findable by its first-message text`
- **TEST-15**: FAIL (harness) — `title editor edits the real title`
- **TEST-14**: PASS — `conversationDisplayLabel.test.ts`, 7/7 (run directly)

`npx playwright test tests/e2e/chat/untitled-conversation-label.spec.ts --workers=1`
→ **4 failed**, twice.

**Where they fail matters:** every failure is inside the SHARED `loginAsAdmin`
helper (`common/auth-helpers.ts:103` and `:144` — `app-setup-username-input` and
then `auth-login-username` never become visible), i.e. **before a single
assertion of mine executes**. The specs' own logic was never reached, so this is
not evidence for or against the feature.

**Control run (the honest check):** an UNTOUCHED, pre-existing spec —
`tests/e2e/chat/chat-basic.spec.ts` — scores **4 passed / 3 failed** on this same
host, with the same login-helper timeouts. So the Playwright harness is
partially broken in this environment independently of this change.

Two rounds of fixes were applied from real run output rather than guessing:
seeding a provider + model so the required `model_id` is supplied (a genuine
defect — every seeded message would otherwise 422), and moving `loginAsAdmin`
out of `beforeEach` into each test to match the pattern of the spec that does
pass. Neither cleared it.

**Therefore: the e2e leg is NOT validated, and phase 8 does not pass.** These
specs must be run on a host with a working e2e harness before this PR is
considered complete on that axis. I am flagging this rather than marking them
PASS or deleting them to make the gate green.

**What IS proven about ITEM-7/10's data dependency**, against the live review
container — the field the whole frontend change reads:

```
GET /api/conversations →
  title    = 'TP53 and Cell Cycle Regulation'
  preview  = 'What does the knowledge base say about the role of TP53 in cell cycle regulation?'
```

So the backend half is verified end-to-end on a real deployment; it is the
browser-level rendering assertions that remain unverified.

`gate:ui (ui): NOT RUN` — `npm run gate:ui` boots the gallery and drives the
runtime-health pass; it depends on the same browser harness that is failing here,
so running it would produce an equally untrustworthy result.

## Live verification (the real proof)

Review container on **:18140** (free port; not 8080 / 18131-18134), running this
branch's musl build, with the GPT-OSS "Free Models" provider and BioGnosia
registered as a SYSTEM MCP server (`is_system=t, is_built_in=f`) per the seeds.

A `manual_approve` conversation, one BioGnosia question, one approval:

```
title after turn 1 (awaiting approval): None
TITLE LANDED after 48s: TP53 and Cell Cycle Regulation
tools that fired: query_rag        ← only query_rag, as required
```

And the bisect proof: with the two hook call sites reverted,
`manual_approve_titles_on_the_first_turn` fails with `left: None` — the exact
reported symptom.
