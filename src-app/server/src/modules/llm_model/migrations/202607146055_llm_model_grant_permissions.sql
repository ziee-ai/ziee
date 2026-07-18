-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_model: grant its domain permissions to the owning group(s) (N9 —
-- these are DOMAIN perms and must NOT live in the auth crate).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['llm_models::downloads_read','llm_models::downloads_cancel','llm_models::downloads_delete'])),
    updated_at = NOW()
WHERE name = 'Users' AND is_system = TRUE;
