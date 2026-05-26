-- Mirror the conversation MCP settings shape onto projects so the same
-- modal can edit both surfaces. Migration 19 added `loop_settings` to
-- `conversation_mcp_settings` and `user_mcp_defaults`; Plan 5 §4 deferred
-- it on the projects table because the v1 raw-JSON editor didn't expose
-- loop tuning. Reusing the per-server tool-toggle modal now requires
-- the project to carry the same field (so it can be snapshotted into
-- new conversations exactly like the other three MCP columns).
ALTER TABLE projects
ADD COLUMN mcp_loop_settings JSONB;

-- Same convention as migration 19: NULL = "not set, application
-- supplies default" so we can distinguish from "explicitly set to
-- defaults". Application-layer default when NULL:
--
--   {
--     "stop_when_no_tool_calling": true,
--     "max_iteration": 10,
--     "stop_when_tools_called": [],
--     "force_final_answer": false,
--     "per_tool_max_iteration": []
--   }
