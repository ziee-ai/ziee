# FIX_ROUND-2 — office-bridge pane RPC

Fixed FIX_ROUND-1's residual findings (committed in f337afb7), then re-audited the
post-fix diff (`/tmp/pane-rpc-v3.diff`) with 2 fresh blind agents.

## Re-audit round (2 blind agents on the FR1-fixed diff)

Both agents confirmed the routing/security/correlation fixes hold. Findings surfaced:

**Confirmed (all fixed in this round's follow-up commit):**
- **[low] classify_pane_frame silent-drop → timeout** — a response frame whose
  `BridgeResponse` deserialization failed (e.g. a partial `{"error":{}}`, since
  `BridgeError.code/message` were required) was dropped with no log, leaving the caller
  to time out. Fixed: `BridgeError.code/message` now `#[serde(default)]` (a partial
  error routes → typed pane error), and the residual parse-failure path is logged.
- **[low] taskpane.js pure helpers untested** (capText marker/slice + sameDoc /
  normPath / isPathLike / baseName) — browser JS, previously manual-only. Fixed:
  guarded the Office bootstrap + exported the pure helpers, and added
  `taskpane.test.mjs` (node) asserting baseName / isPathLike / normPath (incl. the
  Windows `file://` drive-letter case) / sameDoc (cross-dir reject, format+case
  equivalence) / capText (cap + in-band marker).

**Suspected (also fixed, cheap + correct):**
- **[med] normPath Windows drive-letter** — `file:///C:/…` kept a leading slash vs the
  native `C:\…`, so `sameDoc` could FALSELY reject a legit op on Windows. Fixed:
  `normPath` de-slashes a leading-slash drive letter (`/c:/` → `c:/`).
- **[low] resolve_pane case-sensitive** — while the desktop targets (Win/macOS) and the
  JS side treat paths case-insensitively. Fixed: exact + basename match now use
  `eq_ignore_ascii_case`; unit-tested.
- **[low] (None,None) malformed-reply branch untested** — added
  `call_pane_rejects_reply_with_neither_result_nor_error`.

## New confirmed findings: 3

The re-audit surfaced 3 new confirmed (low) findings; all are fixed in the follow-up
commit. Because the round found new confirmed findings, a final convergence round
(FIX_ROUND-3) re-checks the post-fix diff.
