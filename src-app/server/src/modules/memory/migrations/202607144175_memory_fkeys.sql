-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- memory foreign keys (deferred).

ALTER TABLE ONLY public.conversation_memory_settings
    ADD CONSTRAINT conversation_memory_settings_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_default_extraction_model_id_fkey FOREIGN KEY (default_extraction_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_embedding_model_id_fkey FOREIGN KEY (embedding_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.memory_audit_log
    ADD CONSTRAINT memory_audit_log_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_project_id_fkey FOREIGN KEY (project_id) REFERENCES public.projects(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_source_message_id_fkey FOREIGN KEY (source_message_id) REFERENCES public.messages(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_extraction_model_id_fkey FOREIGN KEY (extraction_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
