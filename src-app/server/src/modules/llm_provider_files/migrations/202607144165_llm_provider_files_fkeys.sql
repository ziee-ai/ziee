-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_provider_files foreign keys (deferred).

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_provider_files
    ADD CONSTRAINT llm_provider_files_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;
