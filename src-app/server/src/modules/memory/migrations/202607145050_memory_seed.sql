-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- memory seed data.

-- `enabled` (col 7) seeds OFF: memory is a deployment-wide opt-in (privacy-safe
-- default — see the memory_admin_settings docs, "default OFF"). An admin turns it
-- on via PUT /memory/admin-settings once an embedding model is configured.
-- `fts_enabled` / `semantic_enabled` stay true so both retrieval arms are
-- available once memory is enabled.
INSERT INTO public.memory_admin_settings VALUES (1, NULL, 768, NULL, 8, 0.6, false, '2026-07-14 15:09:57.538587+00', 30, 200, 'simple', true, 60, 4, 0, NULL, NULL, true);
