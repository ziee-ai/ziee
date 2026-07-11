# DECISIONS — tool-artifact-grouping (follow-up #3)

All inputs resolved up front (design dictated by the lead's diagnosis + confirmed in
plan mode). No unresolved markers.

### DEC-1: How does the app actually scroll a virtualized message list?
**Resolution:** `messageListRef.current?.scrollToBottom()` — the `MessageListHandle`
method (`MessageList.tsx:385`, `virt.scrollToIndex(count-1,{align:'end'})` + re-assert).
Native per-element `scrollIntoView` on a virtualized row inside the OverlayScrollbars
viewport is a no-op (the #134 bug).
**Basis:** codebase / lead diagnosis — confirmed the OverlayScrollbars + TanStack-virtual
setup and the existing initial-load use of `scrollToBottom` at `ConversationPage.tsx:272`.

### DEC-2: What triggers the scroll — watch `toolCalls`, or a new store signal?
**Resolution:** Watch `Stores.McpComposer.toolCalls` in `ConversationPage` and scroll for
a new `pending_approval` id (deduped by a ref Set). No new store field / no SSE-handler edit.
**Basis:** convention / minimal blast radius — self-contained in the component that owns
`messageListRef`, reuses existing store state; the dedupe Set fires it once per approval.
A store-counter signal (the task's alternative) would touch the store + `extension.tsx`
handler for no functional gain.

### DEC-3: Bypass the `isAtBottom` gate?
**Resolution:** Yes — call `scrollToBottom()` UNCONDITIONALLY for a new pending approval;
the `isAtBottom` gate is exactly what suppresses the scroll today. Keep only the
conversation-match guard (`conversation?.id === conversationId &&
initialScrollConvIdRef.current === conversationId`) to avoid firing during a stale A→B
switch.
**Basis:** user / lead — the approval needs the user's attention regardless of scroll
position; the whole defect is that the gate stops the follow.

### DEC-4: Respect `prefers-reduced-motion`?
**Resolution:** No special handling. `scrollToBottom()` takes no behavior arg (matches the
existing `messagesEndRef.scrollIntoView` auto-follow). Acceptable per the task.
**Basis:** task — "if `scrollToBottom` doesn't support a behavior arg, that's fine".

### DEC-5: How is the test made a REAL assertion (not a false-green)?
**Resolution:** Reproduce the below-the-fold scenario deterministically (overflow the
list, scroll to top so `isAtBottom===false` — proven by `chat-jump-to-latest-btn` visible),
stream the approval, and assert `toBeInViewport()` on the approval element. Pre-fix this
fails; post-fix it passes.
**Basis:** task / codebase — `jump-to-latest.spec.ts` is the scroll-control template;
`toBeInViewport` asserts the EFFECT, not a `scrollIntoView` call.

### DEC-6: Configurable-settings rule — any operational tunable introduced?
**Resolution:** No. Pure client-side scroll/UX behavior. No limit/retention/rate/toggle/
threshold. N/A.
**Basis:** convention — none of the configurable-settings trigger categories apply.

### DEC-7: Scope — is the single-tool artifact wrapping touched?
**Resolution:** No. Only the approval-scroll wiring (ConversationPage), the dead-scroll
removal (approval component), and the one e2e test are changed.
**Basis:** task — "Don't touch the single-tool artifact WRAPPING (that part of #134 works)."
