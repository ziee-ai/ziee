# FIX_ROUND-5 — re-audit of the FIX_ROUND-4 fixes + gate-boot findings

A blind re-audit (`split-chat-fixround5-audit`, 9 angle-reviews) of the
FIX_ROUND-4 fixes, PLUS a real bug surfaced by the gate:ui dev-server boot that
no prior gate caught. All fixed here; FIX_ROUND-6 re-audits to convergence.

## Confirmed + fixed

- **HIGH — the Chat.store init idempotency guard permanently kills the SINGLETON
  primary's streaming after a destroy→re-init cycle** (state-management,
  re-flagged with a concrete mechanism I had WRONGLY rejected in FIX_ROUND-3). For
  a `defineStore` singleton the STATE OBJECT is created once (store-kit.ts:231) and
  SURVIVES the ref-count destroy (destroy tears down subscriptions, not state), so
  `onCleanup`'s `streamClient.stop()` left the stopped client in state; on re-init
  `if (get().chatStreamClient) return` early-returned and never re-established the
  stream + sync subscriptions. Repro: open chat → navigate away >5s (grace destroy)
  → return → live token streaming + cross-device refetch silently dead. (Local pane
  instances are unaffected — they get a fresh state per mount.) **Fixed:**
  `onCleanup` now `set({ chatStreamClient: null })` after `stop()`, so re-init
  passes the guard and fully re-wires (DRIFT-2.15). This is a regression I
  introduced by moving the stream client from a module-scope flag into per-instance
  state; my FIX_ROUND-3 rejection missed that the singleton's state persists.
- **MEDIUM — a 4th `onStreamError` call site was not threaded** (api-contract) —
  the `sendMessage` catch block (`Chat.store.ts:1830`) called `onStreamError(error)`
  without `get().paneId`, so a synchronous send failure on a split pane resolved
  `composerPaneKey(undefined) → __single__` and could restore into / clobber the
  single-pane slot. **Fixed:** pass `get().paneId` for parity with the other 3
  sites. (Masked today because `onMessageSent`—the only backup site—runs after the
  throwing await, but a real threading gap.)
- **HIGH (machine-gate, not blind-audit) — duplicate `data-testid` literals**
  (`chat-pane-header` / `chat-pane-close`) in BOTH `ConversationPickerPane.tsx` (my
  ITEM-27 addition) and `ConversationPage.tsx`. The vite DEV testid-unique plugin
  REFUSES to boot on this — which is why gate:ui couldn't start the gallery.
  `check:testid-registry` checks registry-sync not cross-file literal uniqueness,
  and e2e uses PREVIEW builds, so only the dev-server boot caught it. **Fixed:** the
  empty picker pane gets its own ids (`chat-picker-pane-header/close`; no spec
  targets them — the pane-close specs fill the pane with a conversation first). The
  plugin now reports 1629 unique ids and the gallery boots. **This is exactly the
  class of defect A7's gate:ui boot canary exists to catch** — a green e2e + green
  `npm run check` both missed it.
- **LOW ×2 — docstring accuracy** — `registry-runtime-per-pane` claimed a "bridge
  Ctrl+Enter send" (it only probes Esc/Ctrl+K, no send) and `mcp-per-pane` claimed
  per-pane chip isolation (the chip is global, DRIFT-2.11). Both docstrings
  corrected to match the assertions.

## Rejected
- (none new)

**New confirmed findings:** 4
