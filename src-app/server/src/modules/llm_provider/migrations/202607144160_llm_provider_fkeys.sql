-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_provider foreign keys (deferred).

ALTER TABLE ONLY public.llm_providers
    ADD CONSTRAINT llm_providers_default_runtime_version_id_fkey FOREIGN KEY (default_runtime_version_id) REFERENCES public.llm_runtime_versions(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_group_id_fkey FOREIGN KEY (group_id) REFERENCES public.groups(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES public.llm_providers(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
