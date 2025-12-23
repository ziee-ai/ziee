-- Assign all built-in LLM providers to all groups for desktop app
-- This ensures users can access all providers without manual assignment

-- Insert provider-group assignments for all built-in providers to all groups
-- ON CONFLICT DO NOTHING handles existing assignments gracefully
INSERT INTO user_group_llm_providers (group_id, provider_id)
SELECT g.id, p.id
FROM groups g
CROSS JOIN llm_providers p
WHERE p.built_in = true
ON CONFLICT (group_id, provider_id) DO NOTHING;
