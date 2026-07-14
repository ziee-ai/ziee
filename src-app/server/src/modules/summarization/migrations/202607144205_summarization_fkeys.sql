-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- summarization foreign keys (deferred).

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_branch_id_fkey FOREIGN KEY (branch_id) REFERENCES public.branches(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.conversation_summaries
    ADD CONSTRAINT conversation_summaries_summarized_up_to_id_fkey FOREIGN KEY (summarized_up_to_id) REFERENCES public.messages(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.conversation_summarization_settings
    ADD CONSTRAINT conversation_summarization_settings_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES public.conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.summarization_admin_settings
    ADD CONSTRAINT summarization_admin_settings_default_summarization_model_i_fkey FOREIGN KEY (default_summarization_model_id) REFERENCES public.llm_models(id) ON DELETE SET NULL;
