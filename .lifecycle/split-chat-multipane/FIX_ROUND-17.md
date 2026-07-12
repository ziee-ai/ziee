# FIX_ROUND-17 — split-chat-multipane (round-8 blind audit: ITEM-57/58)

Blind adversarial review: 3 fresh diff-only reviewers over the round-8 delta
(`git diff 2178f189e..HEAD` — the single-pane edge-drop + desktop tear-off), across
correctness/state/hooks/concurrency · patterns/a11y/design/i18n/responsive ·
tests-quality/scope/error-handling/api-contract/security. Full findings +
dispositions in `LEDGER.jsonl`. No HIGH findings; strong cross-reviewer convergence
on 4 real MEDIUM issues, all fixed.

## Fixed (confirmed)

1. **Right-drop URL↔focus desync + left/right focus asymmetry** (state-management,
   CONFIRMED) + **split-drop bypassed pop-out dedup** (correctness, CONFIRMED). The
   raw `openPane` loop focused the last-opened pane (right→dropped, left→current,
   asymmetric) and left the URL stale, and never consulted `focusPopoutWindowIfOpen`
   so a conversation already live in a native pop-out window got duplicated into a
   pane. **Fix:** route BOTH edges through the canonical
   `openConversationInWorkspace(dropped, {intent:'newPane'})` — it navigates to +
   focuses the dropped conversation (URL == focused pane, symmetric) and dedups the
   pop-out window; a left drop then `reorderPanes` the dropped pane to the front.
   `single-pane-drop.spec.ts` now asserts the URL tracks the dropped conversation.

2. **Tear-off geometry misfires on a degenerate window rect** (correctness +
   tests, PLAUSIBLE-high). A webview reporting `outerWidth/Height = 0` made an EMPTY
   inside-rect → every in-window release read as "outside" → spurious tear-off (and
   pane close) on every drop. **Fix:** `isOutsideWindow` returns `false` for a
   non-positive `outerWidth`/`outerHeight` or any non-finite coord (can't trust the
   geometry ⇒ don't tear off). New unit assertions cover it.

3. **Web-gate e2e was a false-negative + hook/2-of-3-sources/MOVE untested**
   (tests-quality + plan-coverage, CONFIRMED). The old spec passed even if
   `onDragEnd` were never wired (the web path calls nothing regardless). **Fix:**
   rewrote it with a faked-`__TAURI__` positive control asserting `window.open` IS
   called (source → hook → plan → seam) for ALL three sources (card, sidebar item,
   split-pane grip) and that the grip tear-off MOVES (the pane closes), plus the
   web-off + strict-inside negatives.

## Tracked, not fixed (recorded, surfaced to human — FB-15)

- **Esc-cancel-while-outside** and a **bogus `(0,0)` dragend coord** can still
  misfire on desktop — inherent limits of coordinate-based web-DnD that can't be
  cleanly discriminated cross-engine, and are exactly the desktop-webview behavior
  that can't be driven headlessly (same platform-guarantee bucket as TEST-83). The
  degenerate-rect guard kills the worst case; the residual is flagged for
  desktop-host verification, NOT reversed (the human chose the strict-coordinate
  model — DEC-71).
- **MOVE-on-failed-open**: closing the pane when the window open fails mirrors the
  existing ⤢ button (`OpenInNewWindowAction`) verbatim; the conversation is not lost
  (persists in the DB/sidebar, reopenable). A real fix needs the desktop
  `openConversationWindow` seam to report success (ITEM-P1 scope). Tracked FB-15.

## Low / no-change (recorded in LEDGER)

Center label "Replace" (intentional distinct verb) · RTL-readiness of physical
left/right (app is LTR-only) · narrow-width split outcome (handled by SplitChatView's
tab-strip fallback) · unsanitized-but-non-exploitable conversationId in the pop-out
URL · hardcoded MIME literal in the e2e (matches sibling drag-to-split.spec).

**New confirmed findings:** 4
