# BASE — conflict-surface scoping

- **Base branch**: `khoi` (integration; this task branches off and PRs back to
  `khoi`, per the task file). `khoi == main` as of branch time (40d46e5eb).
- **Highest existing migration**: `00000000000157_remove_unused_builtin_mcp_servers.sql`.
  **This branch adds NO migration** (the `google` `auth_providers` row is already
  seeded by migration 47; the encrypted-secret column by migration 125). → no
  migration-number collision possible.
- **OpenAPI regen implied?** **No.** No REST route or response type changes — the
  reconciler runs at boot and writes the DB directly via the existing repository.
  `openapi.json` / `api-client/types.ts` are untouched.
- **Files this branch edits vs. what main is touching**: `modules/desired_state/mod.rs`,
  `config/desired-state.yaml`, `docker-compose.deploy.yml`, `docker/web/README.md`,
  `tests/desired_state/mod.rs`. Single-worker task; no concurrent branch is
  expected to touch these. `config/desired-state.yaml` was last edited by commit
  e597a99d8 (mcp server rename) — additive change here (new top-level key), no
  overlap with the mcp_servers block.
- **Frontend workspaces**: NOT touched → phase-3 e2e gate and phase-8 frontend
  gates do not apply.
