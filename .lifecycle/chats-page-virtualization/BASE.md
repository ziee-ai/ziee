# BASE — conflict-surface scoping

**Branch base:** `origin/main` @ `50b24b6ce` (worktree confirmed up-to-date).

## Migrations
- Highest existing migration: `00000000000157_remove_unused_builtin_mcp_servers.sql`.
- **This feature adds NO migration** (pure frontend rendering-layer change). No
  migration-number collision surface.

## OpenAPI / types regen
- **None.** No backend type, route, or response-shape change → no
  `openapi.json` / `api-client/types.ts` regen. Not treated as backend work.

## Files current main / concurrent work also touches (collision watch)
- **`src-app/ui/src/modules/chat/components/ConversationList.tsx`** — **SHARED with
  claude-live4** (concurrent infinite-scroll paging for the LEFT SIDEBAR recent-chats,
  which also edits this file + `ChatHistory.store.ts`). Mitigation: keep this
  feature's edit a **localized swap of the inner card `.map()` block + a scroller
  ref**, touching no other region; and take **ZERO** edits to `ChatHistory.store.ts`
  (virtualization needs none). Whichever branch merges second reconciles the one
  overlapping region.
- **`src-app/ui/src/modules/chat/stores/ChatHistory.store.ts`** — live4 edits it;
  **this feature does NOT.** No overlap.
- No other file this branch touches is under active concurrent change (the new
  files + `measuredHeightCache.ts` + gallery surfaces are this feature's own).

## Desktop override surface
- `src-app/desktop/ui/` does **not** override the chat module (no
  `ConversationList`/`MessageList` copy) — it shares `src-app/ui/`. So this is a
  **single-workspace** (`src-app/ui`) change; no desktop hand-written counterpart
  to keep in sync. (R2-3 N/A here.)

## Workspace scope
- Touched frontend workspace: `src-app/ui` only. `npm run check (ui)` is the gate;
  `desktop/ui` is not touched.
