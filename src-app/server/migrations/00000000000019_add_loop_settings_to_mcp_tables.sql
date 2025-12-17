-- Add loop_settings column to conversation_mcp_settings
-- This stores per-conversation loop configuration for tool calling iterations

ALTER TABLE conversation_mcp_settings
ADD COLUMN loop_settings JSONB;

-- Add loop_settings column to user_mcp_defaults
-- This stores user-level default loop configuration

ALTER TABLE user_mcp_defaults
ADD COLUMN loop_settings JSONB;

-- Note: We use JSONB without a default value and handle defaults in the application layer
-- This allows us to distinguish between "not set" (NULL) and "explicitly set to defaults"
-- The application will use these defaults when NULL:
-- {
--   "stop_when_no_tool_calling": true,
--   "max_iteration": 10,
--   "stop_when_tools_called": [],
--   "force_final_answer": false,
--   "per_tool_max_iteration": []
-- }
