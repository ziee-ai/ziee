# BASE — conflict-surface scoping

Branch cut from `khoi` (= `origin/khoi`, at `e8b2b0d4a`; `origin/khoi == origin/main` as of
2026-07). PR target: `khoi`.

## Highest existing migration
`00000000000157_remove_unused_builtin_mcp_servers.sql`. **This branch adds NO migration** (no
new table/column) — no migration-number collision surface.

## Files this branch edits that main may also touch
- `src-app/server/src/modules/mcp/resource_link.rs` — SSRF trust-set helper + tests.
- `src-app/server/src/modules/mcp/repository.rs` — new read-only accessor (additive method).
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — trust-set call sites + guidance strings.
- `src-app/server/src/modules/workflow/dispatch.rs` — one trust-set call site.
- `src-app/server/src/modules/code_sandbox/handlers.rs` — tool-description string.
- `src-app/server/tests/mcp/resource_link_test.rs` — new integration test (additive).

All edits are localized and additive/behavioral; none rename public types or touch shared test
harness (`tests/common/**`). Risk of textual merge conflict is low and confined to the mcp module.

## OpenAPI regen
**Not implied.** No request/response schema, route, or OpenAPI-annotated type changes. The new
repository method returns `Vec<String>` internally and is not exposed via any handler. No
`openapi.json` / `api-client/types.ts` regen for either UI workspace.

## Frontend
No `src-app/ui/**` or `src-app/desktop/ui/**` change — the phase-3/phase-8 frontend gates do not
apply to this diff.
