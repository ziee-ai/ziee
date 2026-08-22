-- agent module: run-terminal reconciliation for the durable agent task list.
--
-- Two schema changes backing the fix for the leak where an agent run reaches a
-- terminal state (completed/failed/cancelled/timeout) but its `agent_task_list`
-- rows stay stuck `pending`/`in_progress` forever.
--
-- (1) Widen the status vocabulary with an honest terminal value `abandoned` — a
--     task the run never finished. Distinct from `completed` (would falsely claim
--     the work was done) and from the run-level `cancelled`. Reconciliation flips
--     open rows to `abandoned`; `completed` rows are left untouched.
--
-- (2) `run_id` is POLYMORPHIC (chat = assistant message id; workflow-agent =
--     workflow_runs.id; each fan-out child = a fresh non-persisted id), so it
--     cannot carry a single referential FK. Add a SEPARATE nullable
--     `workflow_run_id` FK to `workflow_runs` that is set ONLY when the run is a
--     real workflow/background run — giving those rows real ON DELETE CASCADE
--     cleanup (the "cascade cleanup on run delete" the original table migration
--     deferred), while chat/fan-out rows keep it NULL and never violate the FK.
--     Mirrors `mcp_tool_calls.workflow_run_id` / `scheduled_task_runs.workflow_run_id`
--     (separate FK column) with the CASCADE of the run-scoped children
--     `background_run_notes` / `file_workflow_runs` (task rows are ephemeral
--     run-scoped working state; history need not survive a run hard-delete).

ALTER TABLE public.agent_task_list
    DROP CONSTRAINT agent_task_list_status_check;

ALTER TABLE public.agent_task_list
    ADD CONSTRAINT agent_task_list_status_check
        CHECK ((status = ANY (ARRAY[
            'pending'::text,
            'in_progress'::text,
            'completed'::text,
            'abandoned'::text
        ])));

ALTER TABLE public.agent_task_list
    ADD COLUMN workflow_run_id uuid
        REFERENCES public.workflow_runs(id) ON DELETE CASCADE;

-- Partial index (matches the mcp_tool_calls.workflow_run_id precedent): chat /
-- fan-out rows keep workflow_run_id NULL, so index only the CASCADE-relevant rows.
CREATE INDEX idx_agent_task_list_workflow_run
    ON public.agent_task_list USING btree (workflow_run_id)
    WHERE workflow_run_id IS NOT NULL;

-- Backfill: link every existing row whose run_id IS a workflow_runs id, so a
-- delete of an already-existing run cascades its task rows too.
UPDATE public.agent_task_list t
SET workflow_run_id = t.run_id
WHERE t.run_id IN (SELECT id FROM public.workflow_runs);
