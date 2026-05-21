-- Grant the `code_sandbox::execute` permission to the default Users
-- group. Idempotent (NOT EXISTS guard) so reruns are safe.
--
-- This permission gates invocation of any code_sandbox tool; without
-- it the user can see the sandbox in their MCP list but `tools/call`
-- returns 403.

DO $$
DECLARE
    target_rows BIGINT;
BEGIN
    -- Count rows that match the target predicate BEFORE the update so
    -- we can distinguish "no matching group exists" (deployment
    -- misconfig) from "permission already granted" (rerun).
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 35: no group matches (name=Users, is_system=true, is_default=true); code_sandbox::execute will NOT be granted. Check that the initial Users group was created by migration 1.';
    END IF;

    UPDATE groups
    SET permissions = array_append(permissions, 'code_sandbox::execute'),
        updated_at = NOW()
    WHERE name = 'Users'
      AND is_system = TRUE
      AND is_default = TRUE
      AND NOT ('code_sandbox::execute' = ANY(permissions));
END $$;
