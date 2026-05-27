-- Grant baseline `memory::read` and `memory::write` permissions to the
-- default Users group so the memory module's REST endpoints
-- (/api/memories, /api/memory/settings) are reachable for normal users
-- without an admin-side opt-in step.
--
-- Mirrors migration 35's idempotent pattern. Admins continue to get
-- `memory::admin::*` via the `*` wildcard on the Administrators group.
-- Without this migration, the memory module is functionally inert for
-- end users — every endpoint returns 403 even though memory is
-- admin-enabled.

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
        RAISE WARNING 'migration 51: no group matches (name=Users, is_system=true, is_default=true); memory permissions will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    -- Baseline user-scoped permissions. Admin permissions
    -- (memory::admin::*) are NOT granted to the Users group — those go
    -- via the Administrators group's `*` wildcard.
    -- memory::core::* grants users management of their own
    -- per-assistant core memory blocks (plan §9 Phase 6, audit R6-#7).
    FOREACH perm IN ARRAY ARRAY[
        'memory::read',
        'memory::write',
        'memory::core::read',
        'memory::core::write'
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
