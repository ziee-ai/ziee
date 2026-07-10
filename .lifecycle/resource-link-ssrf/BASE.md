# BASE — conflict-surface scoping

Branch cut from `origin/khoi` @ `8add2ca5` (integration branch; PR targets `khoi`).

## Highest existing migration
`00000000000132_add_openrouter_provider_type.sql` — **this feature adds NO migration** (no schema
change; the fix is pure request-path logic + a new env var), so no migration-number collision is
possible.

## Files this branch touches that main/khoi may also touch
- `src-app/server/src/modules/mcp/resource_link.rs` — core change. Low collision risk (a focused
  file; recent activity was PR #124/#126 on the mcp approval loop, not this file's fetch branch).
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — 2 call-site edits (add one argument each).
  This file is high-traffic; keep edits minimal and localized to the two `persist_links(...)` calls.
- `src-app/server/src/modules/workflow/dispatch.rs` — 1 call-site edit.
- `src-app/server/tests/mcp/resource_link_test.rs` — test-only.
- `CLAUDE.md` — doc note.

## OpenAPI regen implied?
**No.** No REST route, request, or response type changes; `persist_links` is an internal function.
`openapi.json` / `api-client/types.ts` are untouched → no `just openapi-regen`, no UI workspace
touched → backend-only lifecycle gates apply.
