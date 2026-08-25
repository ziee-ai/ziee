# BASE — conflict-surface scoping

- **Highest existing server migration:** `202608250100`. This feature adds **NO
  migration** (the cap reuses `agent_admin_settings.fan_out_max_threads`; dedup
  uses jsonb equality on the existing `workflow_runs.inputs_json`), so there is no
  migration-number collision surface.
- **OpenAPI regen implied?** NO. No REST handler or schema type changes — the
  guard is internal to the MCP `spawn_background` path, whose result is free-form
  JSON (not in `openapi.json`). No `just openapi-regen` needed.
- **Files this branch edits that main may also touch:**
  - `src-app/server/src/modules/workflow/repository.rs` and `runner.rs` — the
    shared workflow backbone. Edits are ADDITIVE (a new guarded-insert fn + a new
    param/return on `spawn_background_run`) plus one in-module test-call update.
  - `src-app/server/src/modules/background_mcp/tools.rs` + `resume.rs` — the SPAWN
    path. A concurrent agent in a DIFFERENT worktree is editing the background_mcp
    READ paths (`routes.rs` / `handlers.rs`) for a transcript viewer; this branch
    does NOT touch those two files, minimizing merge collisions. The merge-gate
    re-checks against real main.
- **New test file:** `src-app/server/tests/background_mcp/spawn_guard.rs` (+ one
  line in `tests/background_mcp/mod.rs`).
