# BASE — conflict-surface scoping vs current khoi/main

- **Highest existing migration:** `00000000000158_add_users_token_version.sql`.
  This feature adds **NO migration** → no migration-number collision possible.
- **Files this branch edits:** `mcp/chat_extension/helpers.rs`,
  `mcp/chat_extension/mcp.rs`, `tests/mcp/mcp_extension_test.rs`. All in the mcp
  module; no other active workstream is known to be rewriting these on khoi.
- **OpenAPI regen implied?** No. Nothing is added to the serialized `McpServer`
  struct (the `description` column is already serialized); the label + roster are
  runtime-only prompt assembly. No `openapi.json` / `api-client/types.ts` change,
  so the phase-3/phase-8 frontend gates do not apply (backend-only diff).
- **UI workspaces touched?** None (`src-app/ui/**`, `src-app/desktop/ui/**` untouched).
