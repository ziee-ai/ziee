-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- file_rag foreign keys (deferred).

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_file_id_fkey FOREIGN KEY (file_id) REFERENCES public.files(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_embedding_model_id_fkey FOREIGN KEY (embedding_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_reranker_model_id_fkey FOREIGN KEY (reranker_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;
