-- ITEM-6 / DEC-7 / DEC-8: link a fan-out CHILD sub-agent run to its parent chat turn.
--
-- A fan-out child is modeled as its OWN `workflow_runs` row (`job_kind='subagent'`,
-- already a valid job_kind since 202607190700), so it inherits the entire
-- agent-activity persistence stack + the existing workflow-run retention. Two
-- nullable links (every existing / non-chat-parented run leaves both NULL):
--
--   * `parent_message_id` — the parent assistant `message_id`. The QUERY key that
--     `GET /api/subagent-runs?parent_message_id=…` filters on (one chat turn can
--     fan out more than once, so the conversation alone is too coarse). Plain uuid,
--     NOT a FK: `messages` rows are NOT FK-linked to `conversations` and do NOT
--     cascade on a conversation delete (`delete_conversation` relies on FK cascade,
--     and there is none for messages), so a message-FK could NOT guarantee the
--     child cascades with the conversation.
--
--   * `parent_conversation_id` — the parent conversation, `REFERENCES conversations
--     ON DELETE CASCADE`. THIS is the lifecycle guarantee (DEC-3): deleting the
--     conversation cascade-deletes its fan-out child runs. (The row's EXISTING
--     `conversation_id` FK is `ON DELETE SET NULL` — shared by all background runs —
--     so it can't be repurposed for the cascade; a dedicated column is needed.)
--     Child rows are then pruned by the EXISTING workflow-run retention too — no new
--     retention setting (DEC-3/DEC-9).
--
-- No `parent_run_id` is added — no workflow/background host fans out through the
-- isolated child path (DEC-11), so it would be a dead column.
ALTER TABLE public.workflow_runs
    ADD COLUMN parent_message_id uuid,
    ADD COLUMN parent_conversation_id uuid
        REFERENCES public.conversations(id) ON DELETE CASCADE;

-- The children-of-a-turn lookup (partial: only child rows carry the column).
CREATE INDEX idx_workflow_runs_parent_message
    ON public.workflow_runs USING btree (parent_message_id)
    WHERE parent_message_id IS NOT NULL;
