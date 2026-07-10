-- Grant the baseline `voice::transcribe` permission to the default Users group
-- so normal users can use voice dictation (the mic button) without an admin-side
-- opt-in step.
--
-- Mirrors migration 98's idempotent pattern. Admins get voice::admin::* via the
-- `*` wildcard on the Administrators group; this only grants the user-facing
-- `transcribe` perm. Without this migration, POST /api/voice/transcribe returns
-- 403 for every non-admin even when the runtime is set up.

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
        RAISE WARNING 'migration 134: no group matches (name=Users, is_system=true, is_default=true); voice permission will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'voice::transcribe'
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
