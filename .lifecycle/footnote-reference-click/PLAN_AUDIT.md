# PLAN_AUDIT — audit of PLAN.md against the codebase

## Breakage risk

- The only production edit is `useStreamdownComponents.tsx`. The new scoping is a **superset** of the old
  behavior: for a single-prefixed id/href (the shape the old code handled) the helpers produce the exact
  same scoped output (`#${contentId}-fn-1`, `${contentId}-fn-1`), so no existing behavior regresses; for
  the double-prefixed id (currently broken) they now also produce the matching scoped id. Non-footnote
  hash links (`[Section](#section)`) and external links keep their current branches.
- The click handler body (details open + `scrollIntoView`) is untouched — no risk there.
- No backend, migration, OpenAPI, or type change → no `just openapi-regen`, no cross-workspace type drift.
  Desktop UI (`src-app/desktop/ui`) is NOT touched (chat renderer lives only in `src-app/ui`).

## Pattern conformance

- Pure module + colocated `.test.ts` mirrors `imageSrcPolicy.ts`/`imageSrcPolicy.test.ts` — the exact
  pattern `useStreamdownComponents` already references for the `img` policy. ✅
- Reuses `slugifyHeading`/`safeDecode` rather than re-implementing slug logic. ✅
- E2E appends to the existing spec using its own `seedAssistantWithText`/`assistantBubble`/mock helpers
  rather than a new harness. ✅

## Migration collisions

- None. No files under `migrations/` touched; no DB schema involved (footnotes are pure client-side
  markdown rendering).

## OpenAPI regen

- Not required. No request/response types change; `openapi.json` / `api-client/types.ts` untouched.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new pure module, mirrors imageSrcPolicy.ts; reuses markdownHeadings pure fns
- **ITEM-2** — verdict: PASS — a-override rewrite is behavior-preserving for single-prefix, fixes href side already-correct path; no handler change
- **ITEM-3** — verdict: PASS — li-override is the core fix; scopeFootnoteId now matches the click target; superset of old startsWith check
- **ITEM-4** — verdict: PASS — h2 suppression via isFootnoteLabel; catches double-prefixed label; no other h2 affected (non-footnote h2 keep heading path)
- **ITEM-5** — verdict: PASS — node:test unit file under src/**/*.test.ts, picked up by npm run test:unit; mirrors imageSrcPolicy.test.ts
- **ITEM-6** — verdict: PASS — e2e mirrors existing footnote render test; adds click assertions (details[open] + target lookup)
- **ITEM-7** — verdict: CONCERN — needs a two-assistant-message seed; existing seedAssistantWithText seeds one. Resolve by adding a small local two-message seed variant in the spec (mockGetMessages already accepts an array of messages). Not a blocker.
