# BASE — conflict surface vs current main

- **Base commit**: `db2347928` (origin/main at branch creation).
- **Highest existing server migration prefix**: `202607200600`. This branch adds
  **no migration** → no migration-number collision possible.
- **OpenAPI regen implied?** No. No handler signature, request/response type, or
  schema changes. `SandboxAvailability::explain()` is a plain `impl` method (not
  a serialized field), so `openapi.json` / `api-client/types.ts` are unaffected.
- **Files this branch edits that main may also touch**: `code_sandbox` module
  (`handlers.rs`, `version_handlers.rs`, `tools/files.rs`) and the `ziee-sandbox`
  SDK crate (`config.rs`, `tools/execute.rs`, `sandbox.rs`). All edits are
  message-string / log-level / redaction changes, no behavioral/schema surface —
  low merge-collision risk. The SDK crate lives on submodule branch `chat`
  (verified tip `584756d` == `origin/chat`).
- **New module / permission?** None.
