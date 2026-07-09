# PLAN — fix: clicking a footnote/citation reference in a chat message does nothing

## Items

- **ITEM-1**: Add pure, DOM-free helper module `footnoteScope.ts` exposing prefix-count-agnostic
  footnote scoping: `scopeFootnoteId(id, contentId)`, `scopeHref(href, contentId)` (self-contained —
  imports `slugifyHeading`/`safeDecode` directly rather than taking a slugify param), `isFootnoteLabel(id)`.
  Tolerates 0, 1, or 2+ leading `user-content-` clobber prefixes (fixes the Streamdown v2.5 double-prefix)
  and preserves the existing per-message `contentId` scoping + heading-hash retargeting behavior.
- **ITEM-2**: Rewire the `a` override in `useStreamdownComponents.tsx` to compute `scopedId`/`scopedHref`
  via `scopeFootnoteId`/`scopeHref` instead of the inline `startsWith('user-content-...')` logic. Click
  handler (open `.footnote-section` details → open `.footnote-quote` details → `scrollIntoView`) unchanged.
- **ITEM-3**: Rewire the `li` override to scope the footnote definition id via `scopeFootnoteId` (this is
  the id that currently stays double-prefixed and breaks the click target lookup).
- **ITEM-4**: Rewire the `h2` override footnotes-label suppression to `isFootnoteLabel(props.id)` so the
  double-prefixed `user-content-user-content-footnote-label` heading is still suppressed (no stray heading).
- **ITEM-5**: Unit tests for the pure helpers (`footnoteScope.test.ts`, node:test) covering single/double/
  zero prefix, custom label, multi-use suffix, heading hash, external href, `isFootnoteLabel`.
- **ITEM-6**: E2E test — clicking a footnote reference expands the References section, expands the cited
  excerpt, and resolves/scrolls to the correct definition.
- **ITEM-7**: E2E test — per-message scoping: two assistant messages each with `[^1]`; clicking message 2's
  reference targets message 2's References only (message 1 stays collapsed).

## Files to touch

- `src-app/ui/src/modules/chat/core/utils/footnoteScope.ts` (new)
- `src-app/ui/src/modules/chat/core/utils/footnoteScope.test.ts` (new)
- `src-app/ui/src/modules/chat/core/utils/useStreamdownComponents.tsx` (edit: a/li/h2 overrides)
- `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` (edit: append 2 e2e tests; extend seed helper for
  a two-assistant-message scenario)

## Patterns to follow

- **Pure-logic extraction for testability** — mirror `src-app/ui/src/components/common/imageSrcPolicy.ts`
  (+ `imageSrcPolicy.test.ts`): a small pure module referenced from the component, unit-tested with
  node:test. `useStreamdownComponents.tsx` already cites this pattern in its `img` NOTE comment.
- **Reuse existing pure markdown helpers** — `slugifyHeading` / `safeDecode` / `nodeToText` from
  `src-app/ui/src/components/common/markdownHeadings.ts` (already used by `useStreamdownComponents`).
- **node:test unit tests** — mirror the shape of `src-app/ui/src/components/common/imageSrcPolicy.test.ts`
  (imported by `test:unit` via `src/**/*.test.ts`).
- **E2E markdown tests** — mirror the existing `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts`
  (`seedAssistantWithText` + `assistantBubble` + `mockChatStream`/`mockGetMessages`), specifically the
  existing "renders footnotes with collapsed References section" test.
