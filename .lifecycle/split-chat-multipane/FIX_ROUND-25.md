# FIX_ROUND-25 — full-suite-caught regressions from ITEM-72 / ITEM-73

The full 14-split-chat e2e suite (run after the ITEM-73 persistence change) caught
3 failures. All were resolved; 2 were test-model updates, 1 was a real code fix.

## 1. voice-per-pane:83 — REAL code bug (ITEM-72 close×reconcile race) — FIXED

**Symptom (from the failure screenshot):** building `[A|B|C]`, recording in B (middle),
then closing pane 0 (A) produced `[A(recording)|C]` — pane B's slot kept its recording
VoiceStore but its CONVERSATION was replaced with A, and B was dropped.

**Root cause:** clicking a pane's ✕ focuses that pane first (`onPointerDownCapture` →
`focusPane`), so with the ITEM-72 URL-tracks-focused-pane model the address bar briefly
lands on the pane being closed (`/chat/A`). `useClosePane` only navigated when collapsing
to ONE pane, so with ≥2 survivors the URL stayed on `/chat/A`; once `closePane` removed A,
the ITEM-25 URL→workspace reconcile saw a URL pointing at a conversation no longer in any
pane and REPLACED the focused survivor with it (re-adding the closed conversation).

**Fix:** `useOpenConversation.ts::useClosePane` now navigates (`replace`) to the focused
SURVIVOR on every close with ≥2 panes remaining, pinning the URL so the reconcile
converges to `[B|C]`. Blind-audit-analog: the FULL suite caught what TEST-109 (which only
exercised open/focus, never close) could not — recorded as ITEM-72b + LEDGER.

## 2. new-chat-adopt:21 — test encoded the pre-ITEM-72 model — UPDATED

Asserted `page.url() === primaryUrl` after a focused new-chat pane adopts a conversation.
Under ITEM-72 the URL correctly follows the focused pane to its newly-adopted conversation
(NOT a window-hijack — the split stays intact). Assertion updated to prove "no hijack" via
the surviving split + undisturbed other pane, and `url !== primaryUrl`. PASSED isolated.

## 3. split-per-tab-isolation:35 — new-spec timing/robustness — FIXED

My new TEST-111 read sessionStorage before the 250ms debounced save flushed (`seed` null),
and asserted `conversation-title` before the app fully booted. Added the settle wait and
reordered leg A to assert the no-split crux + URL first, then the title.

## Verification

`tsc` green; the full 14-split-chat suite re-run (rebuild) confirms all three green plus
no new regressions from the `useClosePane` core change.

**New confirmed findings:** 0
