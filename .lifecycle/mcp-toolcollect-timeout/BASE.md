# BASE — conflict-surface scoping (P3)

- **Branch base**: `origin/main` @ `b1147a24271dde5179a273e96f9bff86472531de`
  (merge-base == origin/main HEAD; branch is not stale at plan time).
- **Highest server migration prefix in use**: `202608250100`. This branch adds
  **NO migration**, so no collision possible.
- **Duplicate migration prefixes across the tree**: none (`uniq -d` empty).
- **Files this branch edits that main may also touch**:
  - `src-app/server/src/modules/mcp/chat_extension/mcp.rs`
  - `src-app/server/src/modules/mcp/client/stdio.rs`
  - `src-app/server/tests/mcp/{mcp_extension_test.rs,stdio_transport_test.rs}`
  - in-source tests in `src-app/server/src/modules/mcp/client/{stdio.rs,manager.rs}`
  These are stable mcp-module files; no known concurrent main workstream on them.
- **OpenAPI regen implied?** No. No handler signature, response type, or schema
  change — the fix only adds timeout wrapping around existing awaits and one
  in-module helper. `openapi.json` / `api-client/types.ts` unchanged for both ui
  and desktop workspaces.
- **Frontend touched?** No (`src-app/ui/**` / `src-app/desktop/ui/**` untouched) —
  backend-only diff; the FE `npm run check` / `gate:ui` / e2e gates do not apply.
