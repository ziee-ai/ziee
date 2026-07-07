# FIX_ROUND 1

Fixed the confirmed + high-value suspected LEDGER findings (commit 42fa566b), then ran a blind
re-audit of the fix diff.

## Fixes applied (from the phase-6 ledger)
- Chat-extension order 29 → **23** (was colliding with `citations`); comment corrected. (confirmed)
- Session-token store bounded (FIFO cap 64) + `revoke()` + eviction unit test. (confirmed leak)
- POST sinks now enforce the same **Origin allowlist** as `/bridge` (403 before token). (confirmed)
- Watcher `diff_open_docs` no longer keyed so a title-only fallback flips COM identity. (suspected)
- JSON-RPC `tools/call` re-checks runtime `office_bridge_settings.enabled` → typed disabled error. (suspected)
- `connect` wraps the blocking `run_connect` platform calls in `spawn_blocking`. (suspected)
- `edit_document` append rejects missing/blank `text` with INVALID_ARGS + test. (suspected)
- Frontend store gains an `error` state + panel `Alert` error branch; `onError` wired. (confirmed)
- `taskpane.html` token-replace comment corrected; decorative icons get `aria-hidden`. (confirmed/suspected)
- Accepted (documented in code, no behavior change): token in `window.__ZIEE_BRIDGE_TOKEN__` (standard
  add-in same-origin model); CA `BasicConstraints::Unconstrained` safeguarded by discarding the CA key.

Verification: `cargo check -p ziee` + `--tests` green; 38 lib tests pass (incl. new eviction / empty-text /
title-only cases); TEST-7 integration passes; `npm run check` green; TEST-17 (4/4) + TEST-18 pass.

## Blind re-audit of the fix diff — NEW confirmed defect (a regression introduced by fix #4)
- **watcher.rs**: excluding `window_enum_presence` (title-only) entries from the **closed** set was an
  over-correction. For the OPEN case the panel's own refetch still surfaces the doc, but for the CLOSE
  case, a title-only doc that was on-screen and then genuinely closes now emits **no** sync frame, so the
  refetch-driven panel keeps a **ghost/stale entry** until an unrelated event fires (that close WAS emitted
  before the fix). The original "spurious close+open on COM-flip" it was fixing is actually benign for a
  notify-and-refetch panel (a harmless extra refetch), so suppressing closes traded a benign issue for a
  real one. Resolution in round 2: emit open/close for ALL docs (revert the exclusion), documenting that
  the rare COM-flip merely causes an extra (harmless) refetch.

**New confirmed findings:** 1
