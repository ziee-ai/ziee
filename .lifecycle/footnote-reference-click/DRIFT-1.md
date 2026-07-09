# DRIFT-1 — implementation vs plan

Audited the implemented diff against PLAN.md item by item.

- **DRIFT-1.1** — verdict: impl-wins — PLAN ITEM-1 sketched `scopeHref(href, contentId, slugify)`; the
  implemented `scopeHref(href, contentId)` imports `slugifyHeading`/`safeDecode` directly, keeping the
  module self-contained and matching the `imageSrcPolicy.ts` pure-module precedent. PLAN.md ITEM-1
  amended to reflect the two-arg signature (no re-scope of tests — TESTS.md already calls it with two
  args). Resolved.
- **DRIFT-1.2** — verdict: none — ITEM-2 (`a` override) implemented exactly: inline prefix logic replaced
  by `scopeFootnoteId`/`scopeHref`; click handler body unchanged. `safeDecode` import removed from
  useStreamdownComponents.tsx (now only used inside footnoteScope.ts) to avoid an unused-import lint.
- **DRIFT-1.3** — verdict: none — ITEM-3 (`li` override) uses `scopeFootnoteId(id, contentId)`.
- **DRIFT-1.4** — verdict: none — ITEM-4 (`h2` override) uses `isFootnoteLabel(props.id)`.
- **DRIFT-1.5** — verdict: none — ITEM-5/6/7 tests implemented: `footnoteScope.test.ts` (7 node:test cases,
  all pass) + two e2e specs (click-resolves + per-message-scoping) + the stray-heading assertion folded
  into the click spec (TEST-7).

**Unresolved drifts:** 0
