-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_model foreign keys (deferred).

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_repository_id_fkey FOREIGN KEY (repository_id) REFERENCES public.llm_repositories(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_required_runtime_version_id_fkey FOREIGN KEY (required_runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;
