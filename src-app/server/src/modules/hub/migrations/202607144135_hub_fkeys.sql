-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- hub foreign keys (deferred).

ALTER TABLE ONLY public.hub_entities
    ADD CONSTRAINT hub_entities_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE SET NULL;
