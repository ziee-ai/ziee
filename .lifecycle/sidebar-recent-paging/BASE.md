# BASE — conflict-surface scoping vs current main

- **Branch base**: `feat/sidebar-recent-infinite-paging` cut from `origin/main`
  @ `482a9cd05` (verified: `git log` tip is the Merge PR #142 commit that is
  current origin/main at plan time).
- **Highest existing migration**: `00000000000157_remove_unused_builtin_mcp_servers.sql`.
  This feature adds **NO migration** (pure frontend; backend paging already
  exists) → zero migration-number collision risk.
- **OpenAPI regen implied?** **No.** No backend type/route change — the
  conversations list endpoint (`{page,limit,search,sort}` → `{conversations,total}`)
  is used as-is. `openapi.json` / `api-client/types.ts` are untouched.
- **Files this branch touches that main may also be changing**:
  - `src-app/ui/src/modules/chat/stores/ChatHistory.store.ts` — the paging store.
    A chat-history change on main could conflict; it is a self-contained store,
    conflicts (if any) are mechanical. Re-check at merge-gate.
  - `src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx` — the
    sidebar widget.
  - `src-app/ui/src/dev/gallery/seededSurfaces.tsx` + generated gallery files —
    appended entries; regenerated deterministically, low conflict risk.
  - New test files only (no collision).
- **Desktop**: no `src-app/desktop/ui` override of these files → the change flows
  through the `localOverridePlugin` fallback; no desktop file touched, no desktop
  regen implied.
