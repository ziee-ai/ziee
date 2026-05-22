-- Grant local-model download management permissions to the default Users
-- group, so users can manage their own llm_model downloads from onboarding
-- and the model UI.
UPDATE groups
SET permissions = array_append(array_append(array_append(
  permissions,
  'llm_models::downloads_read'),
  'llm_models::downloads_cancel'),
  'llm_models::downloads_delete')
WHERE name = 'Users' AND is_system = TRUE AND is_default = TRUE;
