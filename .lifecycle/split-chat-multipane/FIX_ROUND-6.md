# FIX_ROUND-6 — re-audit of the FIX_ROUND-5 fixes

A blind re-audit (`split-chat-fixround6-audit`, 7 angle-reviews) of the
FIX_ROUND-5 fixes found only TWO LOW findings (the fix-loop is converging:
9 → 5 → 4 → 2). Both fixed; FIX_ROUND-7 re-audits.

## Confirmed + fixed

- **LOW — the `chatStreamClient: null` reset introduced a StrictMode-only,
  dev-only double-client leak for PANE stores** (correctness). The FIX_ROUND-5
  null-set is correct + necessary for the singleton, but its blanket application
  meant that under React StrictMode's synchronous double-invoke of a
  `defineLocalStore` pane's mount effect (on the SAME reused api): init#1 sets
  client A + kicks its async tail; the StrictMode cleanup nulls + stops A; init#2's
  guard now PASSES (because of the null) and spawns client B; init#1's async tail
  resumes and restarts the orphaned A → one idle SSE leaked per pane open. NO
  production impact (prod strips the double-invoke + gives each pane a fresh api),
  but real in dev. **Fixed:** scope the null-set to the SINGLETON only
  (`if (get().paneId == null) set({ chatStreamClient: null })`) — panes never need
  it (fresh state per mount) and are no longer exposed to the StrictMode race, so
  init#2 keeps early-returning as before.
- **LOW — `registry-runtime-per-pane` test TITLE still claimed "Ctrl+Enter"**
  (tests-quality) after the FIX_ROUND-5 docstring correction removed the
  Ctrl+Enter probe. The title over-claimed vs the body (which probes only Esc +
  Ctrl+K). **Fixed:** title now reads "…leaves the survivor's Esc / Ctrl+K
  shortcuts working".

The `ConversationPickerPane` testid rename + the singleton guard were confirmed
CLEAN by every other angle (correctness/state-management/concurrency/api-contract
returned no findings).

**New confirmed findings:** 2
