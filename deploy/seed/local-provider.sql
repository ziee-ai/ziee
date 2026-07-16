-- Deploy seed: add the "Free Models" provider (OpenAI-compatible) that points at
-- the workspace-hosted vLLM stack (GPT-OSS 120B + Llama 4 Scout, behind one
-- LiteLLM endpoint). Idempotent — safe to re-run on every deploy.
--
-- "Free Models" signals to users that these local models cost nothing to use.
-- Base URL uses ziee's OpenAI convention (path ends in /v1, like the built-in
-- OpenRouter provider). The port (4000) is the LiteLLM proxy's published port,
-- reachable via the workspace URL.

-- 0) Rename-safe: an earlier deploy seeded this provider as "Local Provider".
-- Rename it in place so re-running this seed does NOT create a duplicate.
UPDATE llm_providers SET name = 'Free Models' WHERE name = 'Local Provider';

-- 1) The provider. The local LiteLLM proxy needs NO real key, but ziee's UI
-- prompts for one unless a key is set — so store a random dummy key (encrypted
-- with ZIEE_STORAGE_KEY, exactly like the seed does for the google secret).
-- The value is never validated by the proxy.
INSERT INTO llm_providers (name, provider_type, enabled, api_key, api_key_encrypted, base_url, built_in)
SELECT 'Free Models', 'openai', true, NULL,
       pgp_sym_encrypt('sk-local-04a910e8055b6afc2904d3553b8d08d30acf1232', :'storage_key'),
       'https://4000--main--workspace--khoi.workspace.tinnguyen-lab.com/v1', false
WHERE NOT EXISTS (SELECT 1 FROM llm_providers WHERE name = 'Free Models');

-- 2) The models under it. engine_type 'none' = remote API (no local engine).
-- GPT-OSS 120B is ON by default (its vLLM container runs continuously).
INSERT INTO llm_models (provider_id, name, display_name, enabled, is_active,
                        validation_status, engine_type, file_format, capabilities)
SELECT p.id, 'gpt-oss-120b', 'GPT-OSS 120B', true, true, 'valid', 'none', 'safetensors',
       '{"context_length": 65536, "supports_tool_use": true}'::jsonb
FROM llm_providers p
WHERE p.name = 'Free Models'
  AND NOT EXISTS (SELECT 1 FROM llm_models m WHERE m.provider_id = p.id AND m.name = 'gpt-oss-120b');

-- Llama 4 Scout is seeded DISABLED + inactive: its vLLM container is normally
-- stopped to free the GPU, so an active model would only produce errors. The
-- UPDATE below re-asserts "off" on every deploy while the container is stopped.
-- When Scout is brought back online, flip these to true (here + re-enable in UI).
INSERT INTO llm_models (provider_id, name, display_name, enabled, is_active,
                        validation_status, engine_type, file_format, capabilities)
SELECT p.id, 'llama-4-scout', 'Llama 4 Scout (INT4)', false, false, 'valid', 'none', 'safetensors',
       '{"context_length": 65536, "supports_tool_use": true}'::jsonb
FROM llm_providers p
WHERE p.name = 'Free Models'
  AND NOT EXISTS (SELECT 1 FROM llm_models m WHERE m.provider_id = p.id AND m.name = 'llama-4-scout');

UPDATE llm_models m SET enabled = false, is_active = false
FROM llm_providers p
WHERE m.provider_id = p.id AND p.name = 'Free Models' AND m.name = 'llama-4-scout';

-- 3) Make it usable by BOTH the Administrators and Users groups.
INSERT INTO user_group_llm_providers (group_id, provider_id)
SELECT g.id, p.id
FROM groups g, llm_providers p
WHERE g.name IN ('Administrators', 'Users') AND p.name = 'Free Models'
  AND NOT EXISTS (
      SELECT 1 FROM user_group_llm_providers x WHERE x.group_id = g.id AND x.provider_id = p.id
  );
