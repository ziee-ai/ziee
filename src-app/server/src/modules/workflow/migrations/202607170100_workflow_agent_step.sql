-- workflow module: the `kind: agent` step host (ITEM-22 / DEC-8 / DEC-10).
--
-- The agent loop's durable transcript lives on the run row (`agent_transcript_json`,
-- mirroring the elicit resume-field precedent) — it is the resume source for a
-- crash-mid-loop or a durable review-gate suspend. `resumable_agent` is set while
-- the runner is inside an agent step, so the boot sweep can SPARE (not fail) a
-- crashed `running` agent run and re-drive it (ITEM-17). The status CHECK is
-- widened to admit the `resumable` state that sweep marks.

ALTER TABLE public.workflow_runs
    ADD COLUMN agent_transcript_json jsonb,
    ADD COLUMN resumable_agent boolean DEFAULT false NOT NULL;

ALTER TABLE public.workflow_runs
    DROP CONSTRAINT workflow_runs_status_check;

ALTER TABLE public.workflow_runs
    ADD CONSTRAINT workflow_runs_status_check
        CHECK ((status IN ('pending', 'running', 'waiting', 'resumable', 'completed', 'failed', 'cancelled')));
