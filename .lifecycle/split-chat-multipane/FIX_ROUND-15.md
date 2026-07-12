# FIX_ROUND-15 — split-chat-multipane (round-6 blind audit)

Blind adversarial review (1 fresh diff-only reviewer) of the round-6 delta
(ITEM-52 chat-only pop-out route, ITEM-53 focus-existing-window, ITEM-54
snap-back-on-close). Verified-correct: the layout-less `/chat-window/:id` route does
NOT render `AppLayout`, so the main-window snap-back listener never self-registers
in a pop-out window (no close→emit→self-snap loop); the `cancelled`-guarded async
effects handle StrictMode double-mount without leaking; `popoutWindowLabel` is
consistently shared by open + focus; the ITEM-53 web base returns false → zero web
behaviour change.

## Confirmed + fixed

- **HIGH — snap-back never navigated** (`usePopoutSnapBack`). It mutated
  `SplitView.panes` via `openConversationInWorkspace('newPane')` but never
  `navigate`d, yet `SplitChatView` renders ONLY inside `ConversationPage`
  (`/chat/:id`). So if the main window was on a non-chat route
  (new-chat / history / settings) when a pop-out closed, the pane was added but
  nothing rendered it — the conversation silently vanished. Fixed: extracted
  `snapBackAsNewPane` (reconcile-open THEN `navigate('/chat/<id>')`, mirroring the
  sibling `useOpenConversationInWorkspace`), unit-RUN by TEST-84; the listener uses
  it through a navigate ref (registered once, always-current navigate).

## Confirmed — documented (not code-changed)

- **LOW — `atCap` snap-back is a silent skip** (no toast/dialog). Kept: `atCap` is
  a rare edge (popping a pane OUT decrements the count, so snap-back can only hit
  the cap if the user opened MORE panes while the window was open), a modal fired on
  a WINDOW-CLOSE is intrusive, and there is NO data loss (the conversation is one
  sidebar click away). Recorded as a known-limitation.
- **LOW/informational — URL-parse dead branch** in `getSinglePaneConversationId`
  (only consulted at zero panes; safe null on a pop-out path). No change needed.

## Environment note (honest scope)

Every round-6 item ships a test that RUNS its behavior: ITEM-52 render is proven by
a real browser DOM assertion (TEST-79); ITEM-53/54 desktop control flow is run with
the Tauri boundary mocked (TEST-80/83, the established TEST-75 seam pattern) and the
snap-back decision/handler/navigate are pure-run (TEST-81/82/84). The ONE thing not
runnable on this Linux box — the Tauri cross-OS-window event DELIVERY itself — is a
platform guarantee, not owned logic; flagged for desktop-host verification.

**New confirmed findings:** 0
