-- Make the scheduler + notification inbox user-facing: grant the baseline
-- permissions to the default Users group. Mirrors migration 107's idempotent
-- workflow grant.
--
--   scheduler::use      — create/manage own scheduled tasks + run-now + test.
--   notifications::read — list/mark/delete own notifications (strictly
--                         per-user data, so the same perm covers the
--                         per-user mutations — mirrors the citations note).
--
-- Admin-only scheduler::admin::{read,manage} (quota/retention settings) ride
-- the Administrators `*` wildcard and are NOT granted here.

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
        RAISE WARNING 'migration 135: no group matches (name=Users, is_system=true, is_default=true); scheduler/notification permissions will NOT be granted.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'scheduler::use',
        'notifications::read'
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
