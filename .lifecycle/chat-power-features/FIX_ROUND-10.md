# FIX_ROUND-10 — Phase-8 regressions + coverage re-audit

Phase-8 e2e execution + a re-audit of the resulting new hunks. Fixes:

- FIX-25 (state/regression, medium — caught by the history-content-search e2e):
  server-side search returning 0 rows made `conversations` empty, so
  ChatHistoryPage unmounted ConversationList (+ the search box) and showed the
  wrong "No chat history yet" page state. Fixed: keep the list mounted while a
  search is active + suppress the page empty state then.
- FIX-26 (correctness/regression, medium — caught by the jump-to-latest e2e):
  the jump-to-latest / auto-scroll IntersectionObserver effect had `[]` deps, so
  it bailed during the Loading early-return (no sentinel) and never attached once
  the conversation loaded — `atBottom` was stuck true. Fixed by re-attaching on
  `conversation?.id`. (Also repairs the existing streaming auto-scroll suppression.)
- lint fix: `right-3` → logical `end-3` on the find-bar overlay (lint:logical-direction).

Re-audit (2 fresh blind reviewers over the committed diff incl. FIX-25/26):
Reviewer A (FIX-25/26 focus) → NONE. Reviewer B (independent full sweep) → 1 new:

- FIX-27 (security, medium): the fixed `new` localStorage draft bucket was not
  user-scoped and survives logout, so on a shared browser the next user could see
  the previous user's unsent new-chat draft. Fixed: every draft key is now
  namespaced by user id (`makeDraftKey(userId, convId)`), used identically by the
  composer restore/save and the send-time clear. Added a unit test asserting
  cross-user isolation.

Compiles clean (ui tsc; 14 unit tests pass).

**New confirmed findings:** 1
