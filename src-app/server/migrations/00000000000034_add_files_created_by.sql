ALTER TABLE files ADD COLUMN created_by VARCHAR(10) NOT NULL DEFAULT 'user';
-- Values:
--   'user'     — uploaded by the user
--   'llm'      — artifact saved by the LLM via save_as_artifact (code sandbox)
--   'mcp'      — file created by an MCP server (e.g. a chat tool-result resource_link
--                ingested by mcp::resource_link::persist_links)
--   'workflow' — file created by a workflow run's tool step (persist_links with
--                created_by="workflow"); fits VARCHAR(10).
