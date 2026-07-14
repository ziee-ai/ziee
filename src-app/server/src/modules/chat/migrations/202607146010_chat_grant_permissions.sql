-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- chat: grant its domain permissions to the owning group(s) (N9 —
-- these are DOMAIN perms and must NOT live in the auth crate).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['chat::read','chat::create','conversations::create','conversations::read','conversations::edit','conversations::delete','messages::create','messages::read','messages::delete','branches::create','branches::switch'])),
    updated_at = NOW()
WHERE name = 'Users' AND is_system = TRUE;
