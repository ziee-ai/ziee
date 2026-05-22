-- Remove all `hub::models::*` permissions from the default Users group.
-- Browsing and downloading hub models is a privileged action (a download
-- pulls a multi-GB model onto a shared local provider and creates a
-- system-wide model record), so it should not be granted to every user by
-- default. Admins retain access via the Administrators group's `*` wildcard.
--
-- Idempotent: array_remove is a no-op when the permission is already absent,
-- so reruns are safe.

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
        RAISE WARNING 'migration 37: no group matches (name=Users, is_system=true, is_default=true); nothing to remove. Check that the initial Users group was created by migration 1.';
    END IF;

    UPDATE groups
    SET permissions = array_remove(
            array_remove(
                array_remove(permissions, 'hub::models::read'),
                'hub::models::read_version'),
            'hub::models::download'),
        updated_at = NOW()
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE;
END $$;
