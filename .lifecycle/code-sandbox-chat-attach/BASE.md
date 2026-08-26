# BASE — conflict-surface scoping (P3)

Branch `fix/code-sandbox-chat-attach` off `origin/main` @ `ffb5d6130`.

- **Highest existing server migration:** `202608250100`. This branch adds NO
  migration → no migration-number collision possible.
- **Files this branch touches that main may also touch:**
  - `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — a large, frequently
    edited file. This branch adds exactly ONE consumer line in
    `auto_attach_builtin_ids` + one `#[cfg(test)]` test at the end of the test
    module. Low collision surface (append-only test; a single line in a
    well-isolated fn).
  - `src-app/server/src/modules/code_sandbox/mod.rs` — adds one `pub mod` line.
  - `src-app/server/tests/mcp/mcp_extension_test.rs` — appends one test.
  - New files under `code_sandbox/chat_extension/` — no collision (new paths).
- **OpenAPI regen implied?** NO. No handler signature, response type, or schema
  changes. `openapi.json` / `api-client/types.ts` untouched → no regen, no
  desktop regen.
- **Desktop (`src-app/desktop/**`) touched?** NO.
