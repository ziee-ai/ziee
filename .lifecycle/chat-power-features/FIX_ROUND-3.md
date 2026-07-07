# FIX_ROUND-3 — re-audit round 3

Two fresh blind reviewers over the round-2-fixed diff; one reported NONE, the
other surfaced 2 new confirmed findings; both fixed:

- FIX-15 (state-management, medium): onMessageSent cleared the persisted draft on
  EVERY send, so submitting an edit/regenerate (composer overwritten by
  programmatic setText; the user's real unsent draft lives only in localStorage)
  silently deleted that draft. onMessageSent now clears the draft ONLY on a
  normal send — it skips when pendingBranchFromMessageId is set (still set at
  onMessageSent time, cleared right after), so the draft survives an edit/regen
  and the restore effect brings it back when editing ends.
- FIX-16 (perf, low): the collapse useMemo rebuilt the full messageText string
  on every streaming token (O(n^2) over a stream) despite discarding it while
  streaming. It now short-circuits (streaming / just-streamed / active-match →
  false) BEFORE the O(n) concat.

Compiles clean (ui tsc).

**New confirmed findings:** 2
