-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- code_sandbox foreign keys (deferred).

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_base_version_id_fkey FOREIGN KEY (base_version_id) REFERENCES public.file_versions(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.sandbox_workspace_files
    ADD CONSTRAINT sandbox_workspace_files_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;
