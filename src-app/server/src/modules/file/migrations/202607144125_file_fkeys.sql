-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- file foreign keys (deferred).

ALTER TABLE ONLY public.file_versions
    ADD CONSTRAINT file_versions_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_current_version_id_fkey FOREIGN KEY (current_version_id) REFERENCES public.file_versions(id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

-- NOTE (chunk `ziee-file`): the former `files.workflow_run_id` column + its FK
-- to `workflow_runs` moved out of the domain-agnostic store. The file<->run
-- link now lives in the `file_workflow_runs` join table
-- (workflow/migrations/202607144231_workflow_file_runs.sql).
