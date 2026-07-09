-- Grant the knowledge-base permissions to the default Users group so the
-- built-in `search_knowledge` MCP tool + per-user KB CRUD are reachable without
-- an admin opt-in. KBs are per-user data, so normal users both USE the tool and
-- MANAGE their own knowledge bases.
--
-- Mirrors migration 104's idempotent pattern. Admins also hold these via the
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
        RAISE WARNING 'migration 134: no group matches (name=Users, is_system=true, is_default=true); knowledge_base permissions will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'knowledge_base::use',
        'knowledge_base::manage'
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
