# BASE — conflict-surface scoping

- **Base branch**: `origin/khoi` (currently AHEAD of `origin/main`; PR targets `khoi`).
- **Highest existing migration**: `00000000000158_add_users_token_version.sql`.
  This change adds **NO migration** (no schema change), so there is no
  migration-number collision surface.
- **Files this branch edits** (all backend, `src-app/server/**`):
  - `src/modules/code_sandbox/tools/files.rs`
  - `src/modules/code_sandbox/handlers.rs`
  - `tests/code_sandbox/tier3_http.rs` (+ tier2/tier3 test additions)
  None of these are mechanically-generated. No known concurrent worker is editing
  the `code_sandbox` module on `khoi` at plan time.
- **OpenAPI regen implied?** NO. The change is internal to the built-in
  code_sandbox MCP dispatch + file-resolution logic. It adds no REST route, no
  request/response type, no permission. `openapi.json` / `api-client/types.ts` are
  untouched → not treated as UI work by the phase-3/phase-8 frontend gates.
- **Frontend touched?** NO. Backend-only diff → only the backend test chain
  applies at phase 8.
