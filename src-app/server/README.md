# ziee server

Rust + Axum + PostgreSQL backend for the ziee app. See the repo-root
`CLAUDE.md` for the full developer documentation hub.

## Testing

Hub-registry recipes (see repo-root `justfile`):

- `just check-hub` — compile gate. Wipes + re-runs every migration
  against the isolated `hubreg_build` Postgres DB, then `cargo check`
  with `--all-targets` so test code compiles too. Hard-fails if
  Postgres on 127.0.0.1:54321 isn't reachable.
- `just tsc` — runs `tsc --noEmit` against both UI workspaces
  (`src-app/ui` + `src-app/desktop/ui`). Catches API-client drift after
  an OpenAPI regen.
- `just test-hub` — full integration suite for hub install flows
  (`hub::`, `assistant::`, `mcp::`, `llm_model::`). Saves full output
  to `src-app/server/hub-strict-int-<YYYYMMDD-HHMMSS>.log` per the
  CLAUDE.md "ALWAYS Save Full Test Logs" memory. Hard-fails if
  `tests/.env.test` is missing.
- `just ci-hub` — compile gates only (`check-hub` + `tsc` +
  `openapi-regen`). No slow tests; safe to run on every change.

The `hub`, `assistant`, `mcp`, `llm_model` modules together cover the
five install-from-hub flows: MCP user-scope + MCP system-scope +
Assistant user-scope + Assistant template + Model download. See
`tests/hub/catalog_hermetic.rs` for the hermetic mock-Pages tests
(`mock_release_server::spawn_mock_hub` / `MockHub`).
