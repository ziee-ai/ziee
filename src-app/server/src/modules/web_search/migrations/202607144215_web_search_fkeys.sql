-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- web_search foreign keys (deferred).

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
