# FIX_ROUND-3 — v2 blind audit merge + new-spec/new-fix re-audit

Merges the **v2 blind audit** (`LEDGER.jsonl`, 74 findings / 10 angles over the
ITEM-24..39 isolation work), fixes every CONFIRMED finding, and then blind-re-audits
the fixes + the newly-authored e2e specs (`split-chat-fixround3-audit`, 16 angle-
reviews). The re-audit found MORE real defects (a shipped functional bug + spec
bugs + a per-pane completeness gap) — all fixed here; a fresh round (FIX_ROUND-4)
re-audits these fixes to convergence.

## Confirmed from the v2 ledger — fixed

- **`BranchNavigator.tsx:41`** (state-management/correctness) — branch prev/next
  routed to the FOCUSED pane (`Stores.Chat.activateBranch`), corrupting the other
  pane's window on a split. **Fixed:** bind to the render-pane store
  (`useChatPaneOrNull()?.store`), capture id before the await.
- **`File.store` compose→edit backup lost the owner maps** (state-management) —
  restored files were unowned → invisible. **Fixed:** back up/restore the
  `fileOwner`/`uploadOwner` maps (later superseded by the per-pane backup below).
- **4 e2e specs drove pane 1's composer without opening the picker** + the stale
  `mobile-columns` spec — **fixed** (insert `pane-start-new-chat`; replace with
  `mobile-tabs.spec.ts`).

## Confirmed by the FIX_ROUND-3 re-audit — fixed

- **HIGH `SplitChatView.tsx:35` rendered TABS on desktop / COLUMNS on mobile**
  (api-contract, cross-cutting) — `useWindowMinSize().md` is TRUE at ≤768px (main's
  breakpoint-table fix), so `if (!md) return tabs` was inverted; an empirical run
  confirmed `chat-pane-0` is hidden (tab mode) at 1280px. **Fixed:** `if (md) return
  tabs` — desktop tiles columns, ≤768px shows the tab strip. A real shipped
  functional bug (DRIFT-2.12). This root-caused the "columns" spec failures.
- **HIGH `File.store` backup/restore was GLOBAL** (correctness/state-management/
  concurrency/error-handling, 4 angles) — a single whole-store backup slot meant a
  pane's stream-error restore clobbered a concurrently-edited other pane's buffer,
  and two concurrent sends overwrote one slot. **Fixed:** per-pane backup
  (`backupByPane: Map<paneKey, {…}>`); `setBackupFiles`/`restoreFromBackup`/
  `clearBackup` take the sending pane key, snapshot ONLY that pane's owned entries,
  and restore by MERGE (never wholesale-replace); the file-extension hooks pass the
  sending pane key. clearBackup asymmetry (LOW) is resolved by the same rewrite.
- **HIGH spec bugs (my new specs):**
  - `workspace-persist-nav` navigated back to bare `/chat` (→ NewChatPage, no
    split). **Fixed:** nav back to `/chat/${convA}`; deep-link asserts pane counts +
    conversation presence (robust to reload focus-reset), not `ring-primary`.
  - `drag-to-split` asserted the file-negative on pane 1 AFTER the seam drop
    reordered panes to `[C,A,B]` (pane 1 became Alpha). **Fixed:** file-negative runs
    BEFORE the seam drop; the seam-inserted pane is asserted to hold A.
  - `message-actions-per-pane` used `Escape` to "cancel" a destructive edit (no such
    binding) so Regenerate had no target. **Fixed:** Regenerate FIRST, destructive
    Edit LAST; branch nav asserts `/ 2` + a real content step (no vacuous check).
  - `registry-runtime-per-pane` Ctrl+Enter/Ctrl+K were false-green (TextInput's own
    Enter handler; input already focused). **Fixed:** probe the survived GLOBAL
    listener via Esc-clear + blur-then-Ctrl+K-refocus.
  - `mcp-per-pane` asserted per-pane chip isolation, but `McpStatusRow` reads the
    GLOBAL `McpComposer.selectedServers`. **Fixed:** re-scoped to the per-pane config
    SURFACE; per-pane approval routing is unit-proven (DRIFT-2.11).
  - `pane-lifecycle` over-cap asserted only counts. **Fixed:** assert Delta actually
    replaced the focused pane.

## Rejected (false positives)

- `Chat.store.ts:2097` re-init guard — intentional idempotency; store-kit re-init
  gets a fresh state object. Dismissed (see prior analysis).
- `file/chat-extension/extension.tsx:376` `onMessageSent` clears the focused (==
  sending, DRIFT-2.4) pane; the ctx-less hook resolves the sending pane via focus,
  as the amended plan chose. The bounded mid-send focus-move race is DRIFT-2.4;
  now also backed by the per-pane backup keying (each pane's slot is independent).

## Out-of-diff bounded limitation

- `KnowledgeBaseComposer.store.ts` global selection (main-inherited, not in this
  diff) — DRIFT-2.9.

**New confirmed findings:** 9
