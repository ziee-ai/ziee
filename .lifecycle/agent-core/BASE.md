# BASE — conflict-surface scoping (agent-core, on the SDK base)

Base: `origin/main @ 46f605dc5` (post-SDK-extraction). Branch: `feat/agent-core`. Worktree:
`/data/pbya/ziee/tmp/agent-core-wt`. Submodules inited: `sdk/`, `agent-kit/`, `pgvector`.

## Migrations — MODULE-OWNED (SDK N7), not a flat dir
- There is **no `src-app/server/migrations/` flat dir**. Each module owns
  `src/modules/<m>/migrations/<YYYYMMDDNNNN>_<m>_<desc>.sql`; `build.rs` globs `modules/*/migrations/ ∪
  sdk/crates/*/migrations/`, timestamp-sorted. Highest existing counter observed:
  **`202607146095_workflow_grant_permissions.sql`**.
- This branch adds (using the next monotonic `20260716NNNN` counters — H5):
  - `modules/agent/migrations/<ts>_agent_admin_settings.sql` (new `agent` module — greenfield; the
    singleton settings table; NO permission-grant migration — `agent::settings::*` is admin-only via the
    Administrators `*` wildcard).
  - `modules/workflow/migrations/<ts>_workflow_agent_step.sql` (`workflow_runs.agent_transcript_json` +
    `resumable_agent BOOLEAN` + status-CHECK `resumable`).
  - the `mcp_tool_calls.review_classification` column in that table's owning module's migrations.
- **Collision surface** = the `YYYYMMDDNNNN` counter sequence (not a single flat number). Pick counters
  above the current max; the merge-gate C2 re-checks against real main and flags a clash.

## Files this branch touches that main may also be changing
- **`modules/workflow/{validate,types,runner,dispatch,cost,compiled,type_infer,ref_check,models,repository,events}.rs`**
  — additive `StepConfig::Agent` arm + the behaviour-preserving `call_mcp_tool` extraction. Conflicts
  surface at the compiler-checked exhaustive `match` sites.
- **`chat/core/services/streaming.rs` + the chat extensions** — the chat-loop migration (ITEM-24..26) is
  the largest / least-reversible edit; highest regression risk. Guarded by TEST-38/39 (existing chat
  suites unchanged) + TEST-24 parity golden. Watch for main actively editing chat streaming.
- **The app-side sync surface** — a new `SyncEntity` variant (`ziee_framework::SyncEntityKind`) →
  OpenAPI regen.
- **`src-app/Cargo.toml`** (members + `[workspace.dependencies]`) — append-only add of the
  `agent-core` crate; trivial list merge. **`modules/mod.rs`, permissions/settings/sidebar registration**
  — append-only.
- **New `src-app/agent-core/`, `modules/agent/`, `workflow/agent_dispatch.rs`, `ui/src/modules/agent/`** —
  greenfield.

## OpenAPI regen — YES
- New `GET/PUT /api/agent/settings` route + `AgentAdminSettings`/`UpdateAgentAdminSettings` types + the
  new app-side `SyncEntity` variant ⇒ `just openapi-regen` for BOTH `ui/` and `desktop/ui/` (generated
  files excluded from the coverage-law + frontend gates). The `kind: agent` step config is workflow YAML,
  not an HTTP type.

## Permissions — NEW (A9 + A10 apply)
- Introduces **`agent::settings::{read,manage}`** (`ziee_identity::PermissionCheck`; admin-only via the
  Administrators `*` wildcard → **no grant migration**). ⇒ **A9** backend deny test + **A10**
  restricted-user `[negative-perm]` e2e (page/nav absent for a non-admin) — both REQUIRED, enumerated in
  TESTS.md. The agent step itself adds NO new permission (reuses the user's tool RBAC via
  `resolve_tool_server`). `workflow_runs.invocation_source` already permits `'agent'`.

## UI — YES (frontend gates apply)
- Touches `src-app/ui/**` (+ `desktop/ui/**` mirror): agent admin settings page (ITEM-30), workflow
  agent-step authoring + run-view (ITEM-29), plan/todo + progress renderer (ITEM-31), chat parity
  (ITEM-26/31). ⇒ Phase-3 requires `tier: e2e`; Phase-8 requires `npm run check` + `gate:ui` + the
  enumerated e2e (incl. the A10 spec) per touched workspace + gallery coverage for new states.

## SDK interaction
- The `agent-core` crate is a **ziee-workspace** member (NOT an SDK crate); it deps the SDK crates the
  ziee app already deps (`ziee-core`, `ziee-identity`) via the existing cross-workspace path-dep
  mechanism. No SDK (`sdk/`) change is required by this feature. Cross-app driving (§7.2) consumes the
  already-landed `ziee-control-mcp` + the app-side `control_mcp` module — no new SDK work.
- **`ai-providers` stays app-side** (`src-app/server/ai-providers`) — no relocation (design D6).

## New built-in tool surfaces (A8)
- `delegate` (fan-out) + `update_plan` are **core-injected** tools (DEC-4), NOT built-in MCP servers ⇒
  **A8 does not apply** to them (no `mcp.rs` `auto_attach`/`is_builtin` edits needed).

## Scale note
- Large multi-surface feature (crate + workflow + chat migration + admin UI + reviewer). The internal
  build order sequences it; Phase-8 integrated run + the regression/parity tests (TEST-38/39/40) confirm
  the whole system works rather than a green check on an unconsumed crate.
