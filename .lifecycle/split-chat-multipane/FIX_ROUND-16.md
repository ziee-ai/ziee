# FIX_ROUND-16 — split-chat-multipane (round-7 blind audit)

Blind adversarial review (1 fresh diff-only reviewer) of the round-7 delta
(ITEM-55/56 — scope the header's window-management chrome per context), over the
five changed files + the surrounding call sites (`useOpenConversation`,
`SplitView.store`, the route table, `PopoutConversationPage`, the gallery
`MemoryRouter` harnesses, `popoutVisibility.test`).

## Verified correct (nothing to fix)

- **Reactivity** — `Stores.SplitView.panes.length` read in TitleEditor's render
  subscribes on the store-kit proxy (same pattern as `ConversationPage`), so the
  back button appears/disappears as the split opens/closes.
- **`>= 2` split signal** — the workspace is only ever 0 panes (single-pane, via
  `reset()`) or ≥2; `>= 2` exactly matches the `SplitChatView` boundary. Single-pane
  never has ≥2; split never has <2.
- **`useIsPopoutWindow` route prefix** — `/chat-window/` is unique; `/chat`,
  `/chat/:id`, `/chats` don't start with it, and a split pane renders under
  `/chat/:id` → correctly `false`.
- **Rules of Hooks** — `useIsPopoutWindow()` is called unconditionally before every
  early return in all three components.
- **Router safety** — TitleEditor/ConversationPane already used a router hook, so
  adding `useLocation` adds no new constraint; the gallery mounts under
  `MemoryRouter`, so `useLocation` resolves (false).
- **Default arg / regressions** — `popoutActionVisible`'s 3rd arg defaults to false,
  so all existing callers/behaviour are unchanged; single-pane still shows the back +
  split + pop-out buttons.

## Audit method note

Round-7 itself WAS the "audit every affordance in each context" pass the human
demanded (FB-13). Every context-sensitive header/composer action was DRIVEN in
single-pane / split pane / pop-out window and its behavior measured (find +
edit-title verified per-pane; the only defects were window-management chrome in the
wrong context, now fixed). This is the infrastructure-integration walk done for real.

**New confirmed findings:** 0
