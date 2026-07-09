-- Widen workflow_runs.invocation_source to accept 'scheduled'.
--
-- Migration 106 defined the column with an inline CHECK
-- (invocation_source IN ('manual','conversation','agent','mcp_tool')) — the
-- auto-named constraint is workflow_runs_invocation_source_check. The scheduler
-- spawns runs via runner::spawn_run with invocation_source='scheduled', so the
-- CHECK must admit it. Additive/safe (no existing row uses the new value).

ALTER TABLE workflow_runs
    DROP CONSTRAINT IF EXISTS workflow_runs_invocation_source_check;

ALTER TABLE workflow_runs
    ADD CONSTRAINT workflow_runs_invocation_source_check
    CHECK (invocation_source IN ('manual', 'conversation', 'agent', 'mcp_tool', 'scheduled'));
