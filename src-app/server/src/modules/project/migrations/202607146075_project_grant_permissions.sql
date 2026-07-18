-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- project: grant its domain permissions to the owning group(s) (N9 —
-- these are DOMAIN perms and must NOT live in the auth crate).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['projects::create','projects::read','projects::edit','projects::delete'])),
    updated_at = NOW()
WHERE name = 'Administrators' AND is_system = TRUE;
