-- Grant the baseline `control::use` permission to the default Users group so
-- the built-in control MCP tools (list_capabilities / describe_capability /
-- invoke_capability) are reachable for normal users without an admin-side
-- opt-in step. The control server is "enabled for everyone".
--
-- Mirrors migration 98's idempotent pattern. Admins get any future
-- control::admin::* via the `*` wildcard on the Administrators group; this only
-- grants the user-facing `use` perm. Without this migration, the control MCP
-- handler returns 403 for every non-admin call.
--
-- Note: `control::use` only gates ACCESS to the control surface. The ACTUAL
-- per-action authorization is enforced downstream — each invoke_capability
-- dispatches to the real REST route carrying the caller's JWT, so that route's
-- own permission check applies exactly as if the user used the UI.

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
        RAISE WARNING 'migration 126: no group matches (name=Users, is_system=true, is_default=true); control permission will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'control::use'
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
