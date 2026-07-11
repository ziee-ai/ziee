# FIX_ROUND-7 — re-audit of the FIX_ROUND-6 fixes (root-cause the StrictMode edge)

A blind re-audit (`split-chat-fixround7-audit`) found that FIX_ROUND-6's
singleton-scoping of the `chatStreamClient` null-set traded one dev-only
StrictMode issue for another. Root-caused + fixed here; FIX_ROUND-8 re-audits.

## Confirmed + fixed

- **DEV-ONLY (React StrictMode) — the paneId-scoped null-set dropped a pane's
  teardown + sync subscriptions** (correctness / concurrency / state-management —
  the auditor flagged this from all three angles; **no production impact**, since
  React only double-invokes effects in dev). Under StrictMode a pane's mount effect
  runs init#1 → destroy#1 → init#2 on the SAME api: FIX_ROUND-6 skipped nulling
  `chatStreamClient` for panes, so init#2 hit `if (get().chatStreamClient) return`
  and early-returned WITHOUT re-registering the big `onCleanup` teardown; init#1's
  resumed async tail then restarted its stopped client. On the real pane close the
  drained cleanup set no longer held `streamClient.stop()` / `extensionRuntime.
  cleanup()` / `saveConversationState()` / the `sync:conversation`+`sync:reconnect`
  subs → a leaked SSE + a pane that stopped reacting to remote delete/rename.
  FIX_ROUND-6 avoided the double-CLIENT but not this dropped-TEARDOWN.

  **Root-cause fix (DRIFT-2.16):** the real defect (the auditor's own diagnosis) is
  that the init's async tail restarts the client AFTER an `await` with no
  destroyed-guard. Fixed by (a) a per-init-lifecycle `let destroyed = false` flag
  set true FIRST in `onCleanup`, (b) an `if (destroyed) return` in the async tail
  right after `await import('@/modules/auth/Auth.store')` so init#1's tail bails
  instead of restarting the orphaned client, and (c) reverting the null-set to
  UNCONDITIONAL — now safe for panes because the destroyed-guard stops the orphaned
  restart, so init#2 fully re-registers the teardown + sync subs (and the singleton
  navigate-away/return re-wire from FIX_ROUND-5 still holds). One coherent fix that
  closes BOTH the double-client (round-6) and the dropped-teardown (round-7)
  StrictMode edges.

**New confirmed findings:** 1
