-- Grant the baseline `office_bridge::use` permission to the default Users group
-- so the built-in office-bridge MCP tools are reachable for normal users without
-- an admin-side opt-in step.
--
-- Mirrors migration 98's idempotent pattern. Admins get office_bridge::admin::*
-- via the `*` wildcard on the Administrators group; this only grants the
-- user-facing `use` perm. Without this migration, the office_bridge MCP handler
-- returns 403 for every non-admin call.

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
        RAISE WARNING 'migration 133: no group matches (name=Users, is_system=true, is_default=true); office_bridge permission will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'office_bridge::use'
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
