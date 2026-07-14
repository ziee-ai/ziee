-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- assistant_core_memory foreign keys (deferred).

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_assistant_id_fkey FOREIGN KEY (assistant_id) REFERENCES public.assistants(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.assistant_core_memory
    ADD CONSTRAINT assistant_core_memory_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
