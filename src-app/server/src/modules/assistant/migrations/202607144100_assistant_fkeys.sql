-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- assistant foreign keys (deferred).

ALTER TABLE ONLY public.assistants
    ADD CONSTRAINT assistants_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE CASCADE;
