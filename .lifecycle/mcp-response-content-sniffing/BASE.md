# BASE — conflict surface vs current main

Branch cut from `origin/main` @ `7ca09a750`.

- **Highest existing server migration**: `202607200600`. This branch adds NO
  migration, so a prefix collision is structurally impossible.
- **Files this branch touches that main may also be changing**:
  - `src-app/server/src/modules/mcp/client/http.rs` — the single production file.
    It is a large, actively-edited transport module; a concurrent MCP-client
    change on main is the realistic conflict. The edits here are three localized
    hunks (two payload-extraction call sites and one branch body) plus an
    appended `#[cfg(test)]` module, so a textual conflict would be narrow.
  - `src-app/server/tests/mcp/mod.rs` — one added `mod` line; conflicts only if
    main adds a test module in the same position.
  - `src-app/server/tests/mcp/response_framing_test.rs` — new file, no conflict.
- **OpenAPI regen implied**: NO. No handler, route, request/response type, or
  `JsonSchema` derive changes; `openapi.json` / `api-client/types.ts` are
  untouched in both `ui/` and `desktop/ui/`.
- **Frontend touched**: NO.
