-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- voice foreign keys (deferred).

ALTER TABLE ONLY public.voice_runtime_instance
    ADD CONSTRAINT voice_runtime_instance_runtime_version_id_fkey FOREIGN KEY (runtime_version_id) REFERENCES public.voice_runtime_versions(id) ON DELETE SET NULL;
