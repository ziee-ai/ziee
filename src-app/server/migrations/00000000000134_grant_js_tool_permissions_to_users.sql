-- Grant the js_tool (run_js) permission to the default Users group so the
-- built-in `run_js` programmatic-tool-calling tool is reachable without an admin
-- opt-in. `run_js` only exposes tools the conversation already has, in an
-- embedded interpreter with zero ambient capability, and mutating sub-tools
-- still require per-call approval — so it carries the same user-facing risk
-- surface as the model's existing tool access.
--
-- Mirrors migration 104's idempotent pattern. Admins also hold this via the
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
        RAISE WARNING 'migration 134: no group matches (name=Users, is_system=true, is_default=true); js_tool permission will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'js_tool::use'
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
