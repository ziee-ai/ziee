-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- lit_search foreign keys (deferred).

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
