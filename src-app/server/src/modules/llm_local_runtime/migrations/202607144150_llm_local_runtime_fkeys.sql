-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_local_runtime foreign keys (deferred).

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_model_id_fkey FOREIGN KEY (model_id) REFERENCES public.llm_models(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_runtime_version_id_fkey FOREIGN KEY (runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;
