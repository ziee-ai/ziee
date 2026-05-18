UPDATE groups
SET permissions = array_append(permissions, 'code_sandbox::execute')
WHERE name = 'Users'
  AND is_system = TRUE
  AND is_default = TRUE
  AND NOT ('code_sandbox::execute' = ANY(permissions));
