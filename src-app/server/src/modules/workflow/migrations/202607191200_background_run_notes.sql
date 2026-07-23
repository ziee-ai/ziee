-- ITEM-25 (Phase-5 Group F) — durable STEERING-NOTE queue for a running
-- background run (a detached JobKind::SubAgent turn). A user posts a note to a
-- RUNNING run; the detached agent-core loop CONSUMES pending notes at its next
-- iteration boundary and appends them to the transcript so the model reads them
-- on the next turn. The loop-read wiring is a FLAGGED FOLLOW-UP (an agent-core
-- port + a `build_detached_agent_core` impl) — see
-- `workflow::repository::consume_pending_run_notes` + the `AgentCore::run` loop
-- in `src-app/agent-core/src/core.rs`. This tranche ships the durable STORAGE +
-- the enqueue/list REST + the consume seam.
--
-- DRIFT from DEC-79 (task-directed): DEC-79 proposed an IN-MEMORY RunHandle note
-- queue (not persisted). This tranche makes it DURABLE — for a DETACHED run the
-- REST enqueue and the loop-read cross a boundary and must survive a restart, so
-- the note channel is a table FK'd to the run (ON DELETE CASCADE). DEC-79's
-- bounded depth-8 drop-oldest semantics are PRESERVED in `enqueue_run_note`
-- (it trims to the newest 8 pending on insert).
--
-- One durable substrate (`workflow_runs`) still owns the run; this child table
-- carries only the note text + lifecycle timestamps. No denormalized owner —
-- ownership flows through the run (every REST path resolves via
-- `find_run_for_owner`, and ON DELETE CASCADE ties a note's lifetime to its run).

CREATE TABLE public.background_run_notes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    run_id uuid NOT NULL,
    note text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    consumed_at timestamp with time zone,
    CONSTRAINT background_run_notes_pkey PRIMARY KEY (id),
    CONSTRAINT background_run_notes_run_id_fkey
        FOREIGN KEY (run_id) REFERENCES public.workflow_runs (id) ON DELETE CASCADE
);

-- The hot path is "PENDING notes for this run, oldest-first" (the loop consume +
-- the GET list + the drop-oldest trim). A partial index over the unconsumed rows
-- keyed by (run_id, created_at) serves all three.
CREATE INDEX idx_background_run_notes_pending
    ON public.background_run_notes USING btree (run_id, created_at)
    WHERE consumed_at IS NULL;
