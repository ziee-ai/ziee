# BASE — conflict surface vs current main

Branch: `fix/tool-argument-contracts`, cut from `origin/main` @ `db2347928`.

## Migrations

- Highest **server** prefix in use: `202607200600`.
- Highest **desktop** prefix in use: `10000000000005` (the deliberate 1e13 block).
- **This branch adds NO migration.** No schema change, no new table, no grant.
  Migration-number collision surface: none.

## Permissions

No new permission. The two touched surfaces are already gated:
`background::use` (background MCP) and `code_sandbox::execute` (chat sandbox MCP).
No `permissions.rs` edit, no grant migration ⇒ A9/A10 do not apply.

## OpenAPI regen

**Not implied.** The changed schemas are *MCP tool descriptors* — runtime JSON
emitted by `tool_list()` / `tool_definitions()` over JSON-RPC — not aide/schemars
types in `openapi.json`. No handler signature, request type, response type or
`SyncEntity` changes. `openapi.json` / `api-client/types.ts` are untouched in
both workspaces; `types_ts_parity` is unaffected.

## Files this branch touches that main may also be changing

| file | what main is doing | risk |
|---|---|---|
| `src-app/server/src/modules/background_mcp/tools.rs` | recently gained `decode_spec_arg` + `common::tool_args` adoption (already on `db2347928`) | LOW — this branch extends the same block; a concurrent edit would conflict textually in `spawn_background`, which is a 12-line function |
| `src-app/server/src/modules/code_sandbox/handlers.rs` | large, actively edited (2.3k lines) | LOW — the edit is a ~6-line block inside the `execute_command` branch plus one added `#[cfg(test)]` fn |
| `src-app/server/src/modules/code_sandbox/mod.rs` | module init; edited when the sandbox gains providers | LOW — additive helper block only |
| `src-app/server/src/modules/mcp/handlers/mod.rs` | validators | LOW — one fn body replaced by a delegate |
| `src-app/server/src/modules/mcp/user_policy/repository.rs` | policy validation | LOW — one inline check replaced by a delegate |
| `src-app/server/tests/{background_mcp,code_sandbox}/mod.rs` | test module lists | LOW — one added `mod` line each |

## Frontend

No `src-app/ui/**` or `src-app/desktop/ui/**` change. `tsc --noEmit` is still run
in both workspaces as an explicit gate (task requirement), but the phase-3 e2e
requirement and the phase-8 `npm run check` / `gate:ui` gates are not triggered by
this diff.

## Submodules

`sdk/` (which owns `KNOWN_FLAVORS`) is **not** modified — the canonical allow-list
helper is added server-side, next to the existing server-side re-export, so the
branch introduces no submodule pointer bump.
