-- agent module: on-demand `delegate` enable switch (ITEM-2 / DEC-2).
--
-- The core `delegate` tool is offered by agent-core iff the TOP-LEVEL host sets
-- `ToolScope.allow_delegate` (agent-core/src/core_tools.rs). That flag is wired
-- ON only when this admin bool is on (default false), at the two top-level hosts:
--   - the workflow `kind: agent` step (`workflow/agent_dispatch.rs`);
--   - the agent-core chat path (`chat/agent_host/dispatcher.rs`, reached only
--     under ZIEE_CHAT_AGENT_CORE=1 — the legacy chat loop is unaffected).
-- Children / fan-out stay `allow_delegate = false` (the crate's `fanout.rs`
-- structurally caps `max_depth = 1`), and a detached background sub-agent run
-- also stays false (it must not spawn its own sub-agents). It's a plain bool,
-- so no CHECK constraint (mirrors `reviewer_enabled`).
ALTER TABLE public.agent_admin_settings
    ADD COLUMN delegate_enabled boolean DEFAULT false NOT NULL;
