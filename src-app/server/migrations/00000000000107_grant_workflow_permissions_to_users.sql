-- Make workflows user-facing: grant the baseline read + execute permissions
-- to the default Users group so the `/settings/workflows` page is visible and
-- standalone runs work for ordinary users. Mirrors migration 101's idempotent
-- lit_search grant.
--
-- Until now NO migration granted any `workflows::*` to Users, so the whole
-- workflow surface was admin-only via the `*` wildcard on Administrators.
-- Group-assignment (`workflows::assign_to_groups` / `user_can_access`) still
-- scopes WHICH system workflows each user sees; this only grants the baseline
-- read + execute capability.

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
        RAISE WARNING 'migration 104: no group matches (name=Users, is_system=true, is_default=true); workflow permissions will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'workflows::read',
        'workflows::execute'
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
