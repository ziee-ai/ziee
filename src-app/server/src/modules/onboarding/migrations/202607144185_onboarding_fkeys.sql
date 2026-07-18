-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- onboarding foreign keys (deferred).

ALTER TABLE ONLY public.user_onboarding
    ADD CONSTRAINT user_onboarding_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;
