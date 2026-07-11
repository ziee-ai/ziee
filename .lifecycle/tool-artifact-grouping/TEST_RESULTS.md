# TEST_RESULTS — tool-artifact-grouping

Diff touches only the `ui` frontend workspace → the frontend gate chain applies;
no backend chain. Full logs: `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/tag-e2e.log`.

## Static gate

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors + settings-field
  + kit-manifest + testid-registry + design-spec + gallery-coverage + gallery-crawl
  + check-fixtures + state-matrix + overlay-registry all green (exit 0).
- `gate:ui (ui): PASS` — tsc + lint clean; runtime-health crawled all gallery
  cells. **Canary for this diff = clean, proven by base parity.** My isolated run:
  **169/173 surfaces PASS**, failing on EXACTLY the 4 surfaces that also fail on
  `origin/khoi` (base parity run: 168/172 PASS, same 4) — `seeded-llm-models-loading`
  (HIGH 6, crash/console-error in LLM code), `overlay-provider-api-key-modal`
  (HIGH 4, crash), `seeded-s3-group-widget-error` (HIGH 4, console-error), and
  `deep-chat-right-panel-file` (HIGH 2, file-viewer `contrast`). All 4 are
  PRE-EXISTING base debt in code this diff does not touch (their source, gallery
  fixtures, and the mock cassette are byte-identical to base → identical verdict).
  My new surface **`deep-chat-tool-group`** PASSES (only 2 LOW `spacing-grid`,
  informational) and no chat deep-state regressed. → **zero new gate:ui failures
  attributable to this diff.** (The aggregate `gate:ui` command still exits
  non-zero on the pre-existing base failures; an earlier non-isolated run showed 3
  extra `settings-sessions`/`settings-scheduler`/`auth` `request-failed` storms
  that did NOT reproduce in isolation — concurrent-load flake, not this diff.)

## Unit (node:test — `npm run test:unit`, 292 pass / 0 fail total)

- **TEST-1**: PASS — `normalizeToolResultOrder.test.ts` (11 cases: non-adjacent
  artifact pulled adjacent for streaming + reload order, parallel results, lone
  tool, orphan kept in place, non-tool order preserved, multi-result, purity,
  idempotency, identity fast-path).
- **TEST-2**: PASS — `toolRun.test.ts`: `hasArtifactInRun` / `runToolUseIds` /
  `shouldAutoOpen` (running||artifact) / `deriveGroupOpen` (follows userOpen;
  pending_approval forces open).
- **TEST-3**: PASS — `toolRun.test.ts`: `resolveArtifactToolUseId` (prefers event
  id; single-tool_use fallback; single in-flight disambiguation; ambiguous→null;
  in-flight-not-in-message→null / no cross-conversation capture).

## E2E (Playwright, `--workers=1`, isolated ports 19200/19300 — 3 passed, 2.2m)

- **TEST-4**: PASS — `07-mcp/tool-group-artifact-grouping.spec.ts` "a non-adjacent
  artifact tool_result is wrapped in the group box and the group auto-opens".
- **TEST-5**: PASS — `07-mcp/tool-group-auto-open.spec.ts` "a pending approval
  inside a 2-tool group forces the group open (approval actionable)".
- **TEST-6**: PASS — `07-mcp/tool-group-auto-open.spec.ts` "a completed run with no
  artifact is collapsed by default and still expandable".

## Deterministic phase-8 gates (from the diff)

- A2 clean tree: enforced at commit (load-bearing files committed on the branch).
- A3/A4: no diff-added `#[ignore]`/`.skip`/`.only`; no cosmetic/always-true asserts.
- A5: no TEST-ID removed.
- A7: gate:ui (ui) canary recorded (above).
- A8/A9/A10: N/A — no new MCP built-in server, no new permission (this is a pure
  client-side render/UX change).
- R2-5: no new `/api/` route-mock points at a renamed/absent route (the e2e mocks
  reuse the existing `mockGetMessages`/`mockChatTokenStream` routes and the live
  `/api/files/{id}` path).
