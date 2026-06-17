-- Grant the `bio::query` permission to the default Users group.
-- Idempotent (NOT EXISTS guard) so reruns are safe.
--
-- This permission gates calls to the built-in BioMCP server's tools
-- (the `/api/bio/mcp` proxy route extractor). It mirrors how
-- `memory::read` gates the memory MCP route, so JWT auth runs on every
-- bio tool call and admins can revoke biomedical access per group.
-- Administrators already hold it via their `*` wildcard.

DO $$
DECLARE
    target_rows BIGINT;
BEGIN
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 96: no group matches (name=Users, is_system=true, is_default=true); bio::query will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    UPDATE groups
    SET permissions = array_append(permissions, 'bio::query'),
        updated_at = NOW()
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE
      AND NOT ('bio::query' = ANY(permissions));
END $$;
