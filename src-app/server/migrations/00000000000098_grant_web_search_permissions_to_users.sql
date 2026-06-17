-- Grant the baseline `web_search::use` permission to the default Users group
-- so the built-in web_search MCP tools (web_search / fetch_url) are reachable
-- for normal users without an admin-side opt-in step.
--
-- Mirrors migration 61's idempotent pattern. Admins get web_search::admin::*
-- via the `*` wildcard on the Administrators group; this only grants the
-- user-facing `use` perm. Without this migration, the web_search MCP handler
-- returns 403 for every non-admin call even when web search is admin-enabled.

DO $$
DECLARE
    target_rows BIGINT;
    perm TEXT;
BEGIN
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 97: no group matches (name=Users, is_system=true, is_default=true); web_search permission will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'web_search::use'
    ]
    LOOP
        UPDATE groups
        SET permissions = array_append(permissions, perm),
            updated_at = NOW()
        WHERE name = 'Users'
          AND is_system = TRUE
          AND is_default = TRUE
          AND NOT (perm = ANY(permissions));
    END LOOP;
END $$;
