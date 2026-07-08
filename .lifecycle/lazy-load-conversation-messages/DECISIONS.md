# DECISIONS — lazy-load-conversation-messages

All inputs resolved up front so implementation runs nonstop. DEC-1, DEC-2, DEC-3
are the three surfaced to the user for explicit ack (endpoint shape, scroll
anchoring, sizes) per the plan-first pause; the rest are resolved by convention.

### DEC-1: Exact endpoint shape — cursor format, params, and response envelope?
**Resolution:** Extend the existing `GET /conversations/{id}/messages` (do not add
a new route). Query params (all optional, `#[serde(default)]`):
`before: Uuid`, `after: Uuid`, `around: Uuid`, `limit: i64`. At most ONE of
before/after/around may be set (≥2 → 400). No params → the TAIL (newest `limit`).
The **cursor IS a `message_id`** (not an opaque base64 blob): the server resolves
it to that message's `branch_messages.created_at` within the conversation's ACTIVE
branch and keyset-paginates on the composite `(created_at, message_id)`. Response
envelope:
```
PaginatedMessages {
  messages: MessageWithContent[],   // always chronological ASC
  has_more_before: bool,            // older messages exist beyond messages[0]
  has_more_after:  bool,            // newer messages exist beyond messages[last]
}
```
The next cursors are the window endpoints themselves — client sends
`messages[0].id` as the next `before` (scroll-up) and `messages[last].id` as the
next `after` (scroll-down). Unknown / wrong-branch cursor id → 404.
**Basis:** codebase — mirrors `mcp/tool_calls` Query+envelope idiom; message_id-as
-cursor fits the branching model (a cursor is only meaningful inside the active
branch path, and a branch switch naturally invalidates it — which is the correct
semantics). `has_more_before`/`has_more_after` (vs a single `next_cursor`) is
required because `around=` is bidirectional — a single forward cursor cannot
express "more in both directions." Surfaced to user for ack.

### DEC-2: Scroll-anchoring mechanism when prepending older messages?
**Resolution:** Anchor-element + measured-offset restore in `useLayoutEffect`,
reinforced by a short-lived `ResizeObserver`, with `overflow-anchor: none` on the
scroll content. Steps: (1) before fetching older, capture the top-most visible
`[data-message-id]` element and its offset from the viewport top; (2) prepend the
older page (ordered-Map rebuild); (3) in `useLayoutEffect` (pre-paint) re-find the
anchor and set `viewport.scrollTop += (newOffset − savedOffset)` so the view does
not teleport; (4) keep a ResizeObserver armed briefly to re-apply the correction
as late async height (images / katex / mermaid / shiki in the prepended block)
resolves. Viewport element comes from `DivScrollY`'s forwarded
`OverlayScrollbarsComponentRef` (`.osInstance().elements().viewport`); in mobile
`nativeFlow` mode (no OS instance) fall back to `document.scrollingElement`.
**Basis:** convention/codebase — the production-proven reverse-scroll technique;
robust to variable-height tool-result cards and OverlayScrollbars, unlike the two
rejected alternatives: (a) pure `scrollHeight`-delta captured once — teleports
when async content grows after layout; (b) CSS `overflow-anchor` alone —
inconsistent across browsers and defeated by the scroll library. Reuses the
existing `[data-message-id]` attribute + `useLayoutEffect`/IntersectionObserver
idiom already in `ConversationPage.tsx`. Surfaced to user for ack.

### DEC-3: Initial page size, older-page size, around-window size, prefetch threshold?
**Resolution:** initial/tail page = **30**, older page (`before`) = **30**,
newer page (`after`) = **30**, around window = **31** (15 older + target + 15
newer). `limit` param default 30, clamped 1..=100. Reverse-scroll prefetch
threshold = `rootMargin` of **~800px** (≈1.5 viewport heights) on the top-sentinel
IntersectionObserver, so older messages arrive before the user hits the very top.
**Basis:** convention — 30 comfortably fills a tall viewport even with short
messages while keeping the initial payload small for heavy conversations (tool
results / images / katex); the 100 cap bounds a single `around`/`before` payload.
Surfaced to user for ack.

### DEC-4: Change the endpoint response shape, or version it / add a new route?
**Resolution:** Change the existing endpoint's shape in place (bare array →
`PaginatedMessages`). The sole consumer is `Chat.store.ts::loadMessages`, reworked
in lockstep; `tests/chat/messages_test.rs` assertions are updated to read
`.messages`.
**Basis:** codebase — internal API with a single first-party consumer and
regenerated typed clients; a parallel v2 route would leave dead code. No external
API consumers (grep-confirmed).

### DEC-5: Keep the full-history load for AI context, or paginate everywhere?
**Resolution:** Keep `repository/messages.rs::get_conversation_history` (full
branch load) exactly as-is for AI context; add the windowed loader ONLY for the
HTTP/UI path. Pagination is a display concern; summarization/memory/mcp/title/
streaming still receive the complete branch history.
**Basis:** codebase — those 7 callers build the LLM prompt and must not be
truncated. Confirmed in PLAN_AUDIT breakage analysis.

### DEC-6: SSE `complete` / cross-device `reloadOpen` — full reload or merge-tail?
**Resolution:** Merge-tail (upsert the newest page into the existing window)
instead of replacing the whole Map. A user who scrolled up and loaded older pages
keeps them; new turns still append at the bottom. On a branch change during stream
(`branchChangedDuringStream`), reset to the new branch's tail (DEC-8).
**Basis:** codebase — a full replace would silently discard loaded older pages and
re-introduce the "load everything" cost this feature removes. Preserves the
existing stale-guard / `isStreaming` protections in `applyStreamFrame`.

### DEC-7: How does the client read the scroll viewport across desktop/mobile?
**Resolution:** Desktop → `DivScrollY` forwarded ref →
`osInstance().elements().viewport`. Mobile `nativeFlow` (window scroll, no OS
instance) → `document.scrollingElement` / `window`. The anchor capture/restore
uses whichever is active; if neither is resolvable, skip restore (no crash).
**Basis:** codebase — `DivScrollY` already branches on `AppLayout.nativeScroll`;
ITEM-9 must respect both paths (flagged in PLAN_AUDIT).

### DEC-8: Branch switch — reset the window or try to preserve scroll?
**Resolution:** Reset to the new active branch's TAIL on `activateBranch` and on
`branchChangedDuringStream` reconcile. Cursors are only valid within one branch
path, so a switch always refetches the tail and clears `oldestLoadedId`/
`has_more_*`.
**Basis:** convention — matches the existing `activateBranch` → `loadMessages`
refetch; preserving a scroll position across a different message tree is
meaningless.

### DEC-9: Jump-to-message entry point (deep-links / citations / F3)?
**Resolution:** Implement `jumpToMessage(messageId)` in the store (around=) + a
`#message-<id>` URL-hash handler in `ConversationPage` that centers + highlights
the target (reusing the F3 highlight ring / `[data-message-id]`). This is the
enabling primitive; wiring F3's client-side find to SERVER-side search over
unloaded messages is explicitly OUT OF SCOPE (F3 keeps searching the loaded
window for now) and noted as a follow-up alongside virtualization.
**Basis:** user constraint — "design the endpoint for around now, not as a
retrofit." The hash handler is the concrete, testable deep-link surface (TEST-10).

### DEC-10: Virtualization?
**Resolution:** DEFERRED (not in this iteration). Lazy-load only. Documented as a
follow-up in PLAN.md intro and the ITEM-9 comment.
**Basis:** user constraint — variable-height tool-result cards make virtualization
hard; ship lazy-load first.

### DEC-11: New migration or index?
**Resolution:** None. The keyset query uses the existing
`idx_branch_messages_branch_id (branch_id, created_at)` (migration 13).
**Basis:** codebase — that composite index is exactly the window access path.
