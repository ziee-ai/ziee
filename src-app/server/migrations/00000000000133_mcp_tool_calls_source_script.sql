-- js_tool / run_js: admit the new 'script' tool-call source.
--
-- When a `run_js` script calls an MCP tool via its injected `ziee.tools.*` host
-- function, that call re-enters the dispatcher chokepoint (`McpSession::call_tool`)
-- and is recorded in `mcp_tool_calls` like any other, tagged `source='script'`.
-- Without widening the CHECK, the fire-and-forget recording insert would be
-- rejected (and logged as a warning). Mirrors migration 108's widen for
-- 'workflow'. The inline column CHECK from migration 105 is auto-named
-- `<table>_<column>_check` by Postgres.
ALTER TABLE mcp_tool_calls DROP CONSTRAINT mcp_tool_calls_source_check;
ALTER TABLE mcp_tool_calls ADD CONSTRAINT mcp_tool_calls_source_check
    CHECK (source IN ('chat', 'rest', 'always', 'sampling', 'approval', 'workflow', 'script'));
