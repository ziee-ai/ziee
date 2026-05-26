-- Grant the four projects::* permissions to the Administrators system group.
--
-- Pattern mirrors migration 35 (add_code_sandbox_permission). Idempotent
-- (NOT EXISTS guards) so reruns are safe. We do NOT grant to the default
-- Users group here — Chat Projects v1 is opt-in per deployment policy; a
-- follow-up migration mirroring 27_fix_default_user_permissions.sql can
-- broaden later without a schema break.

DO $$
DECLARE
    target_rows BIGINT;
BEGIN
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Administrators'
      AND is_system = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 49: no group matches (name=Administrators, is_system=true); projects::* will NOT be granted. Check that the initial Administrators group was created by migration 1.';
    END IF;

    UPDATE groups
    SET permissions = array_append(permissions, 'projects::create'),
        updated_at = NOW()
    WHERE name = 'Administrators'
      AND is_system = TRUE
      AND NOT ('projects::create' = ANY(permissions));

    UPDATE groups
    SET permissions = array_append(permissions, 'projects::read'),
        updated_at = NOW()
    WHERE name = 'Administrators'
      AND is_system = TRUE
      AND NOT ('projects::read' = ANY(permissions));

    UPDATE groups
    SET permissions = array_append(permissions, 'projects::edit'),
        updated_at = NOW()
    WHERE name = 'Administrators'
      AND is_system = TRUE
      AND NOT ('projects::edit' = ANY(permissions));

    UPDATE groups
    SET permissions = array_append(permissions, 'projects::delete'),
        updated_at = NOW()
    WHERE name = 'Administrators'
      AND is_system = TRUE
      AND NOT ('projects::delete' = ANY(permissions));
END $$;
