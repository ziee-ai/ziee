-- Add context columns to messages table
-- Stores which model, assistant, and MCP servers were used when sending a user message
-- These are soft references (no FK constraints) so they survive deletion of the referenced entities
ALTER TABLE messages
    ADD COLUMN model_id       UUID      NULL,
    ADD COLUMN assistant_id   UUID      NULL,
    ADD COLUMN mcp_server_ids UUID[]    NULL;
