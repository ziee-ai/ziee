ALTER TABLE files ADD COLUMN created_by VARCHAR(10) NOT NULL DEFAULT 'user';
-- Values:
--   'user' — uploaded by the user
--   'llm'  — artifact saved by the LLM via save_as_artifact (code sandbox)
--   'mcp'  — file created by an MCP server (future use)
