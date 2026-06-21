-- E4: link each recorded MCP tool call to the workflow run that made it.
--
-- When a workflow `tool` step calls an MCP tool, the call is recorded in
-- `mcp_tool_calls` like any other (the recording chokepoint is
-- `McpSession::call_tool`). This column lets a run's tool calls be cross-
-- referenced from the workflow run-history surface. ON DELETE SET NULL so
-- deleting a run does NOT cascade-wipe the audit history (mirrors the
-- conversation/server FK policy on this table).
ALTER TABLE mcp_tool_calls
    ADD COLUMN workflow_run_id UUID REFERENCES workflow_runs(id) ON DELETE SET NULL;

-- Widen the trigger-source CHECK to admit the new 'workflow' source (the
-- workflow tool dispatcher). The inline column CHECK from migration 105 is
-- auto-named `<table>_<column>_check` by Postgres.
ALTER TABLE mcp_tool_calls DROP CONSTRAINT mcp_tool_calls_source_check;
ALTER TABLE mcp_tool_calls ADD CONSTRAINT mcp_tool_calls_source_check
    CHECK (source IN ('chat', 'rest', 'always', 'sampling', 'approval', 'workflow'));

-- Cross-reference lookup: a run's tool calls.
CREATE INDEX idx_mcp_tool_calls_workflow_run
    ON mcp_tool_calls(workflow_run_id) WHERE workflow_run_id IS NOT NULL;
