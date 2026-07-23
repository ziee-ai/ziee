# BASE — conflict-surface scoping

## Branch base (NOT main)

This branch is cut off **`origin/khoi`**, not `origin/main`, per the task brief
(PR-1 targets `khoi`; a sibling branch off `origin/deploy-schedule` carries PR-2).

- Base ref: `origin/khoi` @ `68af34059` (Merge pull request #187 from ziee-ai/fix/collapse-border-overlay)
- `origin/main` vs `origin/khoi`: `6	0` (main is 6 commits AHEAD; khoi is 0 ahead) —
  khoi is a strict ancestor of main, so nothing on khoi is unmerged.
- The local `main` ref in this worktree is STALE (it predates the agent-kit submodule
  extraction), so `git diff main...HEAD` yields ~120 KB of unrelated churn. **Every
  lifecycle-check invocation for this feature must pass `--base origin/khoi`** or the
  Phase-6 coverage law will be computed against the wrong diff.

## Migration collisions

**None — this feature ships NO migration.** Migrations are per-module here
(`src-app/server/src/modules/<mod>/migrations/`). The mcp module's highest is
`202607146065_mcp_grant_permissions.sql`; the table this feature reads/writes is
created by `202607140180_mcp_schema.sql`.

The `mcp_settings.approval_mode` / `user_mcp_defaults.approval_mode` DB column
defaults (`202607140180_mcp_schema.sql:56,132`) stay `manual_approve` on both
branches and are deliberately NOT changed: every INSERT in the tree supplies
`approval_mode` explicitly, so the column default is unreachable. No backfill or seed
SQL either (explicit user decision).

## Files main is also touching

`git diff --stat origin/khoi origin/main -- src-app/server/src/modules/mcp src-app/ui/src/modules/mcp`
is **empty** — main has no changes to the MCP backend module or the MCP UI module
beyond khoi. No collision expected on any file in PLAN.md's *Files to touch*.

## OpenAPI regen implied

**Yes.** ITEM-5/6 change two request types (`UpsertMcpSettingsRequest.approval_mode`,
`UpsertUserMcpDefaultsRequest.approval_mode` → `Option`) and ITEM-7 adds a response
field (`UserMcpDefaultsGetResponse.default_approval_mode`). `just openapi-regen` must
run for BOTH workspaces (`src-app/ui` and `src-app/desktop/ui`), and the
`openapi::emit_ts::tests::types_ts_parity` golden test must stay green.

## Cross-branch port

The same logical change lands on `fix/mcp-auto-approve-default-deploy` (off
`origin/deploy-schedule`) as PR-2. Today those two branches diverge on FOUR lines in
four files (`approval/models.rs:122`, `defaults/models.rs:105`, `mcp.rs:2603`,
`settings/repository.rs:21`). ITEM-1..4 collapse that to ONE line
(`ApprovalMode`'s `#[default]` variant), so the cherry-pick has exactly one expected
conflict, resolved to `#[default] AutoApprove` on the deploy side.
