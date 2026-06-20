-- Grant the citation-management permissions to the default Users group so the
-- built-in citations MCP tools + the per-user library are reachable without an
-- admin opt-in. The library is per-user data, so normal users both USE the
-- tools and MANAGE their own library.
--
-- Mirrors migration 98's idempotent pattern. Admins also hold these via the
-- `*` wildcard on the Administrators group.

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
        RAISE WARNING 'migration 104: no group matches (name=Users, is_system=true, is_default=true); citations permissions will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'citations::use',
        'citations::manage'
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
