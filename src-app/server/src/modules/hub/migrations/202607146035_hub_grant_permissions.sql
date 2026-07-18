-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- hub: grant its domain permissions to the owning group(s) (N9 —
-- these are DOMAIN perms and must NOT live in the auth crate).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['hub::assistants::read','hub::assistants::read_version','hub::assistants::create','hub::mcp_servers::read','hub::mcp_servers::read_version','hub::mcp_servers::create'])),
    updated_at = NOW()
WHERE name = 'Users' AND is_system = TRUE;
