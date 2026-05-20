-- Grant the `code_sandbox::execute` permission to the default Users
-- group. Idempotent (NOT EXISTS guard) so reruns are safe.
--
-- This permission gates invocation of any code_sandbox tool; without
-- it the user can see the sandbox in their MCP list but `tools/call`
-- returns 403.

UPDATE groups
SET permissions = array_append(permissions, 'code_sandbox::execute'),
    updated_at = NOW()
WHERE name = 'Users'
  AND is_system = TRUE
  AND is_default = TRUE
  AND NOT ('code_sandbox::execute' = ANY(permissions));
