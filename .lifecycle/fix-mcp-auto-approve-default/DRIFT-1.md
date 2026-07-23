# DRIFT-1 — implementation vs plan

Reconciling the shipped code against PLAN.md after implementing ITEM-1..13.

- **DRIFT-1.1** — verdict: impl-wins — **The sibling `auto_approved_tools` COALESCE was
  already broken, and the plan didn't know.** TEST-14 ("an allow-list survives a later
  PUT that omits it") FAILED on first run: the list read back as `[]`. Cause:
  `serde_json::Value::Null` bound to a jsonb parameter encodes as the JSON value
  `null`, which is NOT SQL NULL, so `COALESCE($4, <table>.auto_approved_tools)` took
  the first arm and overwrote the stored list (then `unwrap_or_default()` on read
  turned JSON null into `[]`). Pre-existing on both branches; the frontend comment at
  `McpComposer.store.ts:410` has been asserting a contract that never held.
  **Resolution:** PLAN.md amended with **ITEM-14** (bind `Option<serde_json::Value>` so
  `None` is a real SQL NULL), PLAN_AUDIT.md given an ITEM-14 verdict, TESTS.md
  extended with **TEST-23** for the user-defaults copy of the same bug. In scope
  because the task's own hard constraint requires that a user who auto-approves
  specific tools still has that persisted and honored — and because it is the same
  statement, the same single row, and needs no migration.

- **DRIFT-1.2** — verdict: impl-wins — **The gallery mock-API cassette needed
  updating; the plan listed no fixture work.** Making `default_approval_mode` a
  REQUIRED response field broke `tsc` on `src/dev/gallery/fixtures/crawl.generated.ts`,
  whose recorded `Mcp.getDefaults` entry was `{}`. **Resolution:** recorded a realistic
  response in `recorded/crawl.json`, and moved the endpoint into the generator's
  existing, documented `LOOSE` set — the set that exists precisely for "a union/enum
  the structural JSON doesn't satisfy exactly" (a JSON import widens an enum to
  `string`). Every other enum-carrying recorded endpoint is already there, so this
  follows the convention rather than bending the harness; the ajv contract test
  remains its gate. Files added to PLAN.md's *Files to touch*.

- **DRIFT-1.3** — verdict: resolved — **ITEM-11 was scoped too broadly, as PLAN_AUDIT
  predicted.** The audit flagged (and DEC-9 resolved) that `saveProjectConfig`'s
  endpoint keeps `approval_mode` REQUIRED. Implemented per DEC-9: only
  `saveConversationConfig` and `saveUserDefaults` omit the field; `saveProjectConfig`
  still sends a value, now sourced from `serverDefaultApprovalMode` instead of the
  `'manual_approve'` literal. No plan change needed — the decision predated the code.

- **DRIFT-1.4** — verdict: resolved — **ITEM-3's `const` → `fn` shape**, exactly as
  PLAN_AUDIT's CONCERN and DEC-3 anticipated. `ApprovalMode::default().to_string()` is
  not const-evaluable, so `DEFAULT_APPROVAL_MODE: &str` became
  `fn default_approval_mode() -> String`, mirroring the file's existing private
  `fn chrono_from_ts`. Both call sites already built owned `String`s.

- **DRIFT-1.5** — verdict: impl-wins — **Doc comments on OpenAPI-exposed types leak
  into the generated TS client.** The first regen pushed ~25 lines of internal Rust
  guidance (branch-divergence policy, migration rationale) into
  `ui/src/api-client/types.ts` as JSDoc on `ApprovalMode` and the two request fields.
  That is noise for a frontend reader and poor API ergonomics. **Resolution:** kept a
  one-line client-facing `///` doc on each and moved the internal rationale to plain
  `//` comments directly above, which schemars does not pick up. Re-ran the regen; the
  `types.ts` delta is now three small, purely client-relevant hunks. No plan change —
  the plan said "regenerate", not "with which comments".

- **DRIFT-1.6** — verdict: none — **`openapi.json`'s ~500-line diff is key-order
  churn, not content.** Verified per the skill's guidance by `sort`ing both versions
  and diffing: the only content deltas are the two `required`-list removals + the
  matching `anyOf`/`"type": "null"` additions, the new `default_approval_mode`
  property + its `required` entry, and the changed descriptions. Exactly the three
  intended schema changes and nothing else.

- **DRIFT-1.7** — verdict: none — **ITEM-4's extraction also feeds the `run_js` gate**,
  which the plan noted only in passing. Confirmed at implementation time
  (`mcp.rs:682` hands the resolved mode to `execute_run_js_call`), so the outer MCP
  gate and the inner `run_js` gate provably resolve identically instead of drifting.
  Recorded in INFRA_INTEGRATION.md; no code or plan change.

- **DRIFT-1.8** — verdict: resolved — **TEST-18's target file changed.** The plan
  named `mcp_extension_test.rs` as the refactor's no-regression guard, but most of
  that file (and all of `mcp_approval_workflow_test.rs`) hard-panics without a real
  LLM API key, and no `tests/.env.test` exists here. Retargeted TEST-18 at
  `chat/title_approval_test.rs` — a stub-LLM + mock-MCP-server test that drives a
  `manual_approve` conversation through a real pending approval — plus the key-free
  cases in `approval_claim_test`, `mcp_approval_loop_test` and `project::mcp_test`.
  These exercise the same gate and DO run. TESTS.md updated; the key-gated exclusion
  is recorded with evidence rather than silently dropped.

- **DRIFT-1.9** — verdict: none — **branch rebased mid-implementation.**
  `origin/khoi` advanced 27 commits while this work was in progress. Rebased onto
  `95ef05ea7`; no conflicts. Verified none of those commits touch this diff's files
  (`git diff --stat 68af34059 origin/khoi` over the mcp backend, mcp UI, gallery, ui
  scripts and mcp tests shows only `ui/scripts/node-test-hooks.mjs`, which this diff
  does not modify). BASE.md's base-ref note still applies, now pointing at the newer
  tip.

**Unresolved drifts:** 0
