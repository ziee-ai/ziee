# TEST_RESULTS — tool-artifact-grouping (follow-up)

Diff touches only the `ui` frontend workspace → the frontend gate chain applies;
no backend chain. Full logs under `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/tag2-*.log`.

## Static gate

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field
  + kit-manifest + testid-registry + design-spec + gallery-coverage + gallery-crawl
  + check-fixtures + state-matrix + overlay-registry all green (exit 0).
- `gate:ui (ui): PASS` — tsc + lint clean; runtime-health crawled all gallery cells.
  My run: **169/173 surfaces PASS**, failing on EXACTLY the 4 PRE-EXISTING base
  surfaces (`seeded-llm-models-loading` HIGH 6, `overlay-provider-api-key-modal`
  HIGH 4, `seeded-s3-group-widget-error` HIGH 4, `deep-chat-right-panel-file`
  HIGH 2) — the same 4 established as pre-existing base debt in the #133 round
  (base-parity run there). This follow-up touches only the MCP tool-grouping /
  approval-scroll code and touches NONE of those 4 surfaces' source/fixtures, so
  their verdict equals base's. The touched chat surfaces (incl. `deep-chat-tool-group`)
  are clean → **zero new gate:ui failures attributable to this diff.**

## Unit (node:test — `npm run test:unit`, 298 pass / 0 fail total)

- **TEST-1**: PASS — `toolRun.test.ts` `shouldWrapRun`: single+artifact→wrap,
  single+multi-artifact→wrap, single+no-artifact→no-wrap, ≥2 tools→wrap,
  empty/orphan→no-wrap.
- **TEST-2**: PASS — `toolRun.test.ts` `shouldWrapRun` determinism + totality (the
  invariant that lets `McpToolUseGroup` and `contentSpan` share one wrap decision).

## E2E (Playwright, `--workers=1`, isolated ports 19200/19300)

- **TEST-3**: PASS — `07-mcp/tool-group-single-artifact.spec.ts` "a single tool with
  ONE artifact is wrapped, auto-opened, and headed by the tool name".
- **TEST-4**: PASS — same spec "a single tool with MULTIPLE artifacts wraps and
  renders every file; a single tool with NO artifact stays a plain card".
- **TEST-5**: PASS — same spec "a pending approval scrolls itself into view when it
  appears" (scrollIntoView spy asserts the `tool-approval-<id>` element scrolled
  with `behavior:'smooth'`).
- **TEST-6**: PASS — reconciled `chat/mcp-resource-links-positioning.spec.ts` (7/7)
  + `mcp-resource-links-streaming.spec.ts` (6/6) green against the new
  wrapped/auto-open layout (helper toggle-click removed; md preview scrolled into
  view before its h1 assertion). The #133 `07-mcp/tool-group-{artifact-grouping,auto-open}`
  specs also stay green.

## Deterministic phase-8 gates (from the diff)

- A2 clean tree: enforced at commit.
- A3/A4: no diff-added `#[ignore]`/`.skip`/`.only`; no cosmetic/always-true asserts.
- A5: no TEST-ID removed.
- A7: gate:ui (ui) canary recorded (above).
- A8/A9/A10: N/A — no new MCP built-in server, no new permission.
- R2-5: the e2e reuses existing route mocks (`mockGetMessages`/`mockChatTokenStream`)
  + the live `/api/files/{id}` path — no renamed/absent route.
