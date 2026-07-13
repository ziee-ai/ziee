# FIX_ROUND-23 — blind audit of ITEM-72 (workspace→URL sync, FB-19)

Scope: the single new hunk of this round — the second `useEffect` in
`ConversationPage.tsx` (workspace→URL: navigate the address bar to the focused
pane's conversation) plus the `useNavigate` import + `focusedConvId` derivation.

## Blind adversarial review (diff-only fork, no author reasoning)

A fresh/blind reviewer got ONLY the diff hunk and was told to assume nothing.
Angles exercised: navigation loop / ping-pong (the primary risk), state-management /
Rules-of-Hooks, history pollution (replace vs push), and edge cases
(split closing 2→1, `conversationId: null` new-chat pane, external deep-link URL
change, `focusedPaneId` pointing at a pane not in `panes`).

**Verdict: NO CONFIRMED FINDINGS.** The change is loop-safe and correct. Each risk
was cleared with a concrete trace:

- **Loop/ping-pong (primary):** cleared. Focus change → `focusedConvId` changes →
  new effect navigates → `conversationId` updates → URL→workspace reconcile runs
  and no-ops (`focused.conversationId === conversationId`). Deep link (reverse):
  `conversationId` changes → reconcile focuses the matching pane → `focusedConvId`
  changes → new effect no-ops via its equality guard. The new effect's deps
  `[focusedConvId, panes.length]` **exclude `conversationId`**, so an external URL
  change never fires it — the two directions cannot ping-pong.
- **Hooks/state-management:** cleared. `focusedConvId` is derived with a plain
  `panes.find(...)` at component top level from the already-reactive
  `panes`/`focusedPaneId` destructure — NOT a store proxy read inside a
  loop/conditional (the class of bug that caused the ITEM-70 overlay crash). The
  `exhaustive-deps` disable is legitimate.
- **`focusedPaneId` stale / no matching pane:** cleared. `find(...)?.conversationId
  ?? null` → `null` → `if (!focusedConvId) return`. No crash, no wrong nav.
- **Split closing 2→1:** cleared. Effect early-returns on `panes.length < 2`; the
  close-pane hook owns that navigation → no double-nav.
- **History:** `{ replace: true }` is correct (matches DEC-73 — focus changes must
  not push history entries).

## LOW note (dispositioned, NOT a defect)

The reviewer noted: focusing an EMPTY new-chat pane (`conversationId: null`) leaves
the URL on the previously-focused conversation rather than pointing at `/chat`.

**Disposition — WONTFIX (current behavior is correct; the "fix" would be a bug).**
`/chat` (no `:conversationId`) routes to `NewChatPage`, a DIFFERENT component from
`ConversationPage`. Navigating there while a split is open would UNMOUNT the split.
So leaving the URL unchanged when the focused pane is empty is the SAFE, correct
behavior — it keeps the split mounted. DEC-73 scopes the sync to "the focused
pane's *conversation*"; an empty pane has no conversation identity to mirror. No
change made.

## Did NOT re-audit already-confirmed-real parts

Only the new hunk was in scope this round; the rest of the feature diff is
unchanged from the prior converged rounds.

**New confirmed findings:** 0
