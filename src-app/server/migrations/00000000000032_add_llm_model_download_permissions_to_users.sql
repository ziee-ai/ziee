UPDATE groups
SET permissions = array_append(array_append(array_append(
  permissions,
  'llm_models::downloads_read'),
  'llm_models::downloads_cancel'),
  'llm_models::downloads_delete')
WHERE name = 'Users' AND is_system = TRUE AND is_default = TRUE;
