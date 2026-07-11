# BASE — conflict-surface scoping

- **Base branch:** `origin/khoi` (currently `origin/khoi == origin/main` @ 906b6d2a). PR targets
  `khoi`.
- **Highest existing migration:** `00000000000154_add_voice_streaming_settings.sql`. This change
  adds **NO migration** (frontend-only) → no migration-number collision possible.
- **OpenAPI regen:** NONE. No backend types change; `openapi.json` / `api-client/types.ts` untouched.
- **Files this branch edits that main may also touch:** `Chat.store.ts`, `MessageList.tsx`,
  `ChatMessage.tsx` are hot chat-module files. Risk: another in-flight chat worker
  (`feat/chat-toolresult-pairing` worktree exists on the same base). Mitigation: the edits here are
  localized to the `complete` SSE handler + the empty-completion gate threading; re-run `merge-gate`
  before merge to catch any textual collision. `check:state-matrix` may report line-number drift in
  these files — regen + commit if so.
- **Desktop `ui/` overrides (R2-3):** `src-app/desktop/ui/` mirrors chat components. This fix is
  pure client render/store logic (no security/permission logic). Diff the desktop counterpart of
  any touched file; if the desktop carries its own copy of `Chat.store.ts`/`emptyCompletion.ts`,
  apply the equivalent change. (Verify during Phase 5 whether desktop re-exports the server-ui chat
  module or holds its own copy.)
