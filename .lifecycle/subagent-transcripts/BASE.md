# BASE — conflict surface vs current main

## Highest existing server migration prefix
`202608210100` (`modules/agent/migrations/202608210100_agent_task_list_reconcile.sql`).
This feature's new migration MUST sort ABOVE it (server 2026… sequence, NOT the
desktop 1e13 block). Highest within the `workflow` module dir: `202607191200`.

## Files this branch touches that main may also change
- `modules/workflow/{repository,types,agent_dispatch,routes,handlers}.rs` — core workflow
  module; stable on main recently (last workflow churn was background_run_notes). Low collision risk.
- `modules/background_mcp/tools.rs` — recently STABLE; the last edits (B-OWN/B-BACK campaign)
  touched `mcp/chat_extension/mcp.rs` + `mcp/client/manager.rs`, NOT background_mcp. Low risk.
- `agent-core/src/{core,fanout,ports}.rs` — the shared crate. `core.rs` is a 20-field struct
  literal — adding `child_sink_factory` breaks EVERY construction site (documented trap in
  CLAUDE.md); MUST update all sites incl. `agent-core/tests/real_llm_loop.rs`. Verify with
  `cargo check -p agent-core --tests` (NOT `-p agent-core` alone).
- `modules/chat/agent_host/dispatcher.rs` — one construction site of AgentCore; touched here only
  to inject the factory.
- `ui/src/modules/{background,chat}/...` + `ui/src/modules/workflow/components/run/AgentActivityTimeline.tsx`
  (reuse, read-only ideally).

## OpenAPI regen implied? YES
New response types: `BackgroundRunDetail.activity`, the subagent-runs list/detail DTOs.
Run `just openapi-regen` (BOTH ui + desktop) at fan-in; golden parity test guards it.

## Desktop
Active UI is `src-app/ui`; `src-app/desktop/ui` has NO matching component files for these
surfaces (per research). Desktop UI edits likely none beyond regen'd types.
