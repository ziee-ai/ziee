-- Deploy seed: add the "Local Provider" (OpenAI-compatible) that points at the
-- workspace-hosted vLLM stack (GPT-OSS 120B + Llama 4 Scout, GPU 2, behind one
-- LiteLLM endpoint). Idempotent — safe to re-run on every deploy.
--
-- Base URL uses ziee's OpenAI convention (path ends in /v1, like the built-in
-- OpenRouter provider). The port (4000) is the LiteLLM proxy's published port,
-- reachable via the workspace URL.

-- 1) The provider (empty api_key — the local proxy needs none).
INSERT INTO llm_providers (name, provider_type, enabled, api_key, base_url, built_in)
SELECT 'Local Provider', 'openai', true, NULL,
       'https://4000--main--workspace--khoi.workspace.tinnguyen-lab.com/v1', false
WHERE NOT EXISTS (SELECT 1 FROM llm_providers WHERE name = 'Local Provider');

-- 2) The two models under it. engine_type 'none' = remote API (no local engine).
INSERT INTO llm_models (provider_id, name, display_name, enabled, is_active,
                        validation_status, engine_type, file_format, capabilities)
SELECT p.id, 'gpt-oss-120b', 'GPT-OSS 120B', true, true, 'valid', 'none', 'safetensors',
       '{"context_length": 65536, "supports_tool_use": true}'::jsonb
FROM llm_providers p
WHERE p.name = 'Local Provider'
  AND NOT EXISTS (SELECT 1 FROM llm_models m WHERE m.provider_id = p.id AND m.name = 'gpt-oss-120b');

INSERT INTO llm_models (provider_id, name, display_name, enabled, is_active,
                        validation_status, engine_type, file_format, capabilities)
SELECT p.id, 'llama-4-scout', 'Llama 4 Scout (INT4)', true, true, 'valid', 'none', 'safetensors',
       '{"context_length": 65536, "supports_tool_use": true}'::jsonb
FROM llm_providers p
WHERE p.name = 'Local Provider'
  AND NOT EXISTS (SELECT 1 FROM llm_models m WHERE m.provider_id = p.id AND m.name = 'llama-4-scout');

-- 3) Make it usable by BOTH the Administrators and Users groups.
INSERT INTO user_group_llm_providers (group_id, provider_id)
SELECT g.id, p.id
FROM groups g, llm_providers p
WHERE g.name IN ('Administrators', 'Users') AND p.name = 'Local Provider'
  AND NOT EXISTS (
      SELECT 1 FROM user_group_llm_providers x WHERE x.group_id = g.id AND x.provider_id = p.id
  );
