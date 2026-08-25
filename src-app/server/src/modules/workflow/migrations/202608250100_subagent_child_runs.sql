-- ITEM-6 / DEC-7 / DEC-8: link a fan-out CHILD sub-agent run to its parent chat turn.
--
-- A fan-out child is modeled as its OWN `workflow_runs` row (`job_kind='subagent'`,
-- already a valid job_kind since 202607190700), so it inherits the entire
-- agent-activity persistence stack + the existing workflow-run retention. The ONLY
-- new linkage a chat-parented child needs is the parent assistant `message_id`:
--
--   * it is the key `GET /api/subagent-runs?parent_message_id=…` filters on;
--   * the FK `REFERENCES messages(id) ON DELETE CASCADE` makes the child row
--     cascade-delete with its parent assistant message — and, transitively, with
--     the conversation (a conversation delete cascades its messages). This is how
--     "child rows CASCADE-delete with their parent" holds (DEC-3) WITHOUT a new
--     retention setting: child rows are ordinary `workflow_runs` rows and are
--     pruned by the existing workflow-run retention.
--
-- Nullable: every existing run, and every non-chat-parented run, leaves it NULL.
-- No `parent_run_id` / `parent_conversation_id` is added — only chat fan-out is
-- wired to persist children (DEC-7/DEC-11), so those columns would be dead.
ALTER TABLE public.workflow_runs
    ADD COLUMN parent_message_id uuid REFERENCES public.messages(id) ON DELETE CASCADE;

-- The children-of-a-turn lookup (partial: only child rows carry the column).
CREATE INDEX idx_workflow_runs_parent_message
    ON public.workflow_runs USING btree (parent_message_id)
    WHERE parent_message_id IS NOT NULL;
