# FIX_ROUND-2 — re-audit round 2

Two fresh blind reviewers over the round-1-fixed diff surfaced 3 new confirmed
findings; all fixed:

- FIX-9 (state-management, medium): in-app A→B conversation switch reused
  TextInput without remount; restore bailed on leftover text so B's draft never
  loaded and A's text bled into B. Restore now keys off draftKey CHANGES via a
  ref, replacing the textarea with the target key's draft exactly once per key.
- FIX-10 (a11y, medium): clamped content kept focusable descendants clipped +
  alpha-faded (WCAG 2.4.7/2.4.11). `onFocusCapture` now auto-expands the block
  when a descendant gains focus while clamped.
- FIX-11 (tests-quality, low): the collapse e2e now measures the content
  region's clamped (≤400px) vs expanded height, not just the toggle label.

Compiles clean (ui tsc; server unchanged).

**New confirmed findings:** 3
