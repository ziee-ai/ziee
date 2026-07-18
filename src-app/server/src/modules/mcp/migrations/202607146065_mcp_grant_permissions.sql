-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- mcp: grant its domain permissions to the owning group(s) (N9 —
-- these are DOMAIN perms and must NOT live in the auth crate).

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['mcp_servers::read','mcp_servers::create','mcp_servers::edit','mcp_servers::delete'])),
    updated_at = NOW()
WHERE name = 'Users' AND is_system = TRUE;

UPDATE groups
SET permissions = ARRAY(SELECT DISTINCT unnest(permissions || ARRAY['mcp_user_policy::edit'])),
    updated_at = NOW()
WHERE name = 'Administrators' AND is_system = TRUE;
