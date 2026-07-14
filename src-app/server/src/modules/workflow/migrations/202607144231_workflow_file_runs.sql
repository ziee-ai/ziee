-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- file <-> workflow-run join table (chunk `ziee-file`).
--
-- The generic `ziee-file` store is domain-agnostic and carries NO run column.
-- The file<->run association the workflow module needs (A3 run-artifact linking,
-- A5 run-delete cascade, run-history surfacing) lives here as an explicit join.
-- ON DELETE CASCADE on both sides: deleting a run drops its join rows (the A5
-- cascade still explicitly deletes the run-CREATED files first via
-- `list_file_ids`); deleting a file drops its join rows.

CREATE TABLE public.file_workflow_runs (
    file_id uuid NOT NULL,
    workflow_run_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.file_workflow_runs
    ADD CONSTRAINT file_workflow_runs_pkey PRIMARY KEY (file_id, workflow_run_id);

ALTER TABLE ONLY public.file_workflow_runs
    ADD CONSTRAINT file_workflow_runs_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.file_workflow_runs
    ADD CONSTRAINT file_workflow_runs_workflow_run_id_fkey FOREIGN KEY (workflow_run_id) REFERENCES public.workflow_runs(id) ON DELETE CASCADE;

CREATE INDEX idx_file_workflow_runs_run ON public.file_workflow_runs USING btree (workflow_run_id);
