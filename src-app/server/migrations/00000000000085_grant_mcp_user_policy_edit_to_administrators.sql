-- Explicit grant of `mcp_user_policy::edit` to the Administrators
-- system group.
--
-- The Administrators group already holds the `*` wildcard
-- (migration 1), so this permission is already effectively granted
-- via wildcard coverage. We add the explicit entry so:
--
--   1. The permission is visible in migration history alongside the
--      Rust `McpUserPolicyEdit` declaration (migration 84 + the
--      `permissions.rs` change in the same feature).
--   2. A future tightening of the wildcard wouldn't silently
--      strip admin policy-edit rights.
--   3. Permission audit tools that enumerate explicit grants don't
--      show a phantom "ungranted admin perm".
--
-- Idempotent — only appends when not already present.

DO $$
DECLARE
    target_rows BIGINT;
BEGIN
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Administrators'
      AND is_system = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 85: no group matches (name=Administrators, is_system=true); mcp_user_policy::edit will NOT be granted. Check that the initial Administrators group was created by migration 1.';
    END IF;

    UPDATE groups
    SET permissions = array_append(permissions, 'mcp_user_policy::edit'),
        updated_at = NOW()
    WHERE name = 'Administrators'
      AND is_system = TRUE
      AND NOT ('mcp_user_policy::edit' = ANY(permissions));
END $$;
