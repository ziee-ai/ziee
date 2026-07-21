-- Backbone generalization (ITEM-14 / DEC-22/23, Phase-5 LOCK-2 hybrid).
--
-- Generalize `workflow_runs` into the SINGLE durable background-run substrate:
-- a background run (a detached sub-agent turn, a fire-and-forget sandbox exec)
-- reuses the runner's spawn / liveness-heartbeat / RunHandle / startup-sweep
-- machinery but has NO backing `workflows` row and NO on-disk `workflow.yaml`.
--
--   * `workflow_id` becomes NULLABLE — a non-workflow run has no bundle. The FK
--     (`workflow_runs_workflow_id_fkey`, ON DELETE CASCADE) still holds for the
--     non-NULL case; a NULL is simply un-referenced.
--   * `job_kind` is a NEW ORTHOGONAL discriminator — NOT run_kind (normal/test/
--     dry_run) and NOT invocation_source (manual/conversation/agent/...):
--       'workflow'      — the classic YAML-DAG run (default; full back-compat).
--       'sandbox_exec'  — a fire-and-forget background command.
--       'subagent'      — a detached agent-core turn (JobKind::SubAgent).
--
-- No separate `background_jobs` table (LOCK-2 invariant): one durable substrate,
-- one orphan-sweep, one status model, one SSE / sync / notify / retention
-- pipeline. A new kind plugs in via the decentralized `JobKind` policy registry
-- (workflow/job_kind.rs), never a schema change.
--
-- CHECK constraints added via ADD CONSTRAINT (the DROP+re-add pattern of
-- 202607170100 applies when WIDENING an existing CHECK; job_kind is a fresh
-- column so a plain ADD CONSTRAINT suffices).

ALTER TABLE public.workflow_runs
    ALTER COLUMN workflow_id DROP NOT NULL;

ALTER TABLE public.workflow_runs
    ADD COLUMN job_kind text DEFAULT 'workflow'::text NOT NULL;

ALTER TABLE public.workflow_runs
    ADD CONSTRAINT workflow_runs_job_kind_check
        CHECK ((job_kind IN ('workflow', 'sandbox_exec', 'subagent')));

-- Coherence guard (DEC-22 "no fake ephemeral workflow row"): a classic
-- 'workflow'-kind run MUST keep a real bundle FK; only the generalized
-- background kinds may carry a NULL workflow_id. All pre-existing rows satisfy
-- this (job_kind defaults 'workflow' and workflow_id was previously NOT NULL).
ALTER TABLE public.workflow_runs
    ADD CONSTRAINT workflow_runs_job_kind_workflow_id_check
        CHECK (
            (job_kind = 'workflow' AND workflow_id IS NOT NULL)
            OR job_kind <> 'workflow'
        );

-- Index the discriminator: the per-kind boot orphan-sweep and the background-run
-- retention prune both filter by `job_kind`.
CREATE INDEX idx_workflow_runs_job_kind ON public.workflow_runs USING btree (job_kind);
