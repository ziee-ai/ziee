# BASE — conflict-surface scoping (agent-orchestration)

> P3 conflict-surface record. This branch (`feat/agent-orchestration`) is off
> `feat/agent-core` (not `origin/main`) — it inherits the shared agent-core loop,
> `fanout.rs`, and the workflow `kind: agent` host. Scope is a MENU (Phase 1
> only); this records what CURRENT main / the base branch touches that a chosen
> build would also touch, so a migration-number or file collision is visible now.

## Base branch
- Branch: `feat/agent-orchestration`, cut from `feat/agent-core`.
- `feat/agent-core` already carries: `src-app/agent-core/` (the crate), `chat/agent_host/`, `workflow/agent_dispatch.rs`, the `agent` module (`agent_admin_settings`), and the `ZIEE_CHAT_AGENT_CORE` flag. A merge to `main` must reconcile with wherever agent-core lands on main first.

## Migrations
- Numbering is **timestamp-style** `YYYYMMDDHHMM_<name>.sql` in each **module's own** `migrations/` dir (composed at build via `compose_merged_migrations`); there is no central `src-app/server/migrations/`.
- Highest existing on this branch: **`202607170105_mcp_review_classification.sql`** (mcp), with `202607170100_workflow_agent_step.sql` (workflow) and `202607160100_agent_admin_settings.sql` (agent) just below.
- Any new table (a `background_jobs` table for Option B, a `SelfPaced` column for ITEM-21, sub-agent run rows) uses a **fresh future timestamp** in the owning module's `migrations/` — low collision risk given the timestamp scheme, but pick a timestamp later than the current max at author time.

## Files likely also touched by main / the base branch (watch for churn)
- `src-app/agent-core/**` — actively evolving on `feat/agent-core`; ITEM-1/2 edit `fanout.rs`/`core.rs`/`types.rs` — coordinate with any in-flight agent-core change.
- `chat/agent_host/**` and `workflow/agent_dispatch.rs` — same base branch; ITEM-2/5 touch both.
- Chat SSE-event + content-block **compose seams** — additive per-module enums (proc-macro), so low direct-collision risk, but a clean-build check (B4) is required for new variants.
- `code_sandbox` + the **`sdk` submodule** (`sdk/crates/ziee-sandbox`, pinned `9e6d8c74`) — a Group-C build bumps the submodule pin; that is a cross-repo change, higher coordination cost.
- `scheduler/**` and `src-app/ui/src/modules/scheduler/**` — a shipped feature; ITEM-21/22 extend it (additive), reuse its FE.

## OpenAPI regen implied?
- **Yes, for most non-trivial scope:** new REST (background-job status, scheduler self-paced/bind inputs) or new types require `just openapi-regen` for **BOTH** `ui/` and `desktop/ui/` (verify `npm run check` in both). Pure agent-core-internal + chat-SSE work (delegate tool, compose-seam events) may not add REST surface — confirm per chosen scope.
- Desktop embeds the server, so the scheduler tick loop + any background backbone already run in-process on desktop; no desktop kill-switch to add for those, but new chat UI needs desktop parity.
