-- Ephemeral, conversation-scoped workflows (LLM-authored, run-from-workspace).
--
-- A chat model can author a workflow.yaml (+ scripts) into its per-conversation
-- sandbox workspace and run it via the workflow_mcp `run_from_workspace` verb.
-- The runner requires a `workflows` row, so each such run materializes an
-- EPHEMERAL row that:
--
--   1) `ephemeral = TRUE` — excluded from every listing (never surfaces as a
--      `wf_<slug>` MCP tool nor on the workflows page); it only ever runs via
--      the generic verb.
--   2) `conversation_id` — the owning conversation. `ON DELETE CASCADE` GCs the
--      row (and, transitively, its runs) when the conversation is deleted, so
--      an LLM-authored throwaway workflow never outlives its conversation.
--
-- Both columns are additive and nullable/defaulted; existing rows are unchanged
-- (`ephemeral = FALSE`, `conversation_id = NULL`).

ALTER TABLE workflows
    ADD COLUMN ephemeral BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN conversation_id UUID REFERENCES conversations(id) ON DELETE CASCADE;

-- List queries filter `ephemeral = FALSE`; a partial index keeps that cheap and
-- lets the per-conversation cleanup path find a conversation's ephemeral rows.
CREATE INDEX idx_workflows_conversation
    ON workflows(conversation_id) WHERE ephemeral = TRUE;
