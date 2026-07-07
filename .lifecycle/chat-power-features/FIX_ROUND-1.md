# FIX_ROUND-1 — merge ledger → fix → re-audit (round 1)

## Round-0 ledger findings (10 real, 2 design-accepted) → fixes applied

- FIX-3 (correctness/security, LIKE metachars): `escape_like` in the handler
  escapes `\ % _` so the search term matches literally (ILIKE default ESCAPE `\`).
  Regression test `test_search_escapes_like_metacharacters` added.
- FIX-1/FIX-2 (state-management): `conversation.created` now prepends to
  `recentConversations` (deduped) unconditionally, but only optimistically
  inserts into the main list + bumps `total` in the UNFILTERED, `recent`-sort
  view — no filtered-list pollution or inflated total.
- FIX (concurrency): added a `reloadQueued` trailing-refetch so a search/sort
  change during an in-flight load is re-run when it settles, not dropped.
- FIX-4 (perf): `ChatMessage` memoizes the message-text length + collapse
  decision so the find-highlight re-render doesn't recompute them per message.
- FIX-5 (patterns): `CollapsibleBlock` fade switched from a `to-background`
  color overlay to a background-agnostic `mask-image` alpha fade.
- FIX-6 (a11y): find bar restores focus to the find-toggle button on close.
- FIX-7 (a11y): `aria-controls` + a `useId` region id on the collapse toggle.
- FIX-8 (i18n): paste-too-large copy no longer double-spaces when the pasted
  image has no filename.
- ACCEPTED (perf ILIKE no-index — DEC-3; Cmd/Ctrl-F override — DEC-5): documented
  design decisions, not fixed.

## Round-1 re-audit (2 fresh blind reviewers over the fixed diff) → 3 new confirmed

- FIX-9 (state-management, medium): in-app A→B conversation switch reuses
  TextInput (no remount); the old restore guard bailed on leftover text so B's
  draft never loaded and A's text bled into B. Restore now keys off draftKey
  CHANGES (a ref), replacing the textarea with the target key's draft exactly
  once per key.
- FIX-10 (a11y, medium): clamped content kept focusable descendants clipped +
  alpha-faded (WCAG 2.4.7/2.4.11). Resolved: `onFocusCapture` auto-expands the
  block when a descendant gains focus while clamped.
- FIX-11 (tests-quality, low): the collapse e2e only checked the toggle label;
  it now measures the content region's clamped vs expanded height.

All fixes compile (server `cargo check --tests` clean; `ui` `tsc` clean;
`npm run check (ui)` green; 13 unit tests pass).

**New confirmed findings:** 3
