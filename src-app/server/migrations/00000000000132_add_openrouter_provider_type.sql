-- Add OpenRouter as a first-class provider type.
--
-- OpenRouter is OpenAI-compatible for chat (routes through the OpenAIProvider
-- client) and exposes a rich PUBLIC, keyless `/api/v1/models` catalog that the
-- discovery endpoint parses for context window + capabilities.

-- Replace the inline CHECK constraint from migration 3 to admit 'openrouter'.
-- The inline constraint gets the conventional auto-generated name
-- `llm_providers_provider_type_check`; drop-if-exists keeps this idempotent
-- across environments where it may have been named differently.
ALTER TABLE llm_providers
    DROP CONSTRAINT IF EXISTS llm_providers_provider_type_check;

ALTER TABLE llm_providers
    ADD CONSTRAINT llm_providers_provider_type_check CHECK (
        provider_type IN (
            'local', 'openai', 'anthropic', 'groq', 'gemini',
            'mistral', 'deepseek', 'huggingface', 'custom', 'openrouter'
        )
    );

-- Seed the built-in OpenRouter provider (disabled until an admin enables it),
-- mirroring the built-in rows seeded in migration 3.
INSERT INTO llm_providers (name, provider_type, enabled, built_in, base_url)
    VALUES ('OpenRouter', 'openrouter', false, true, 'https://openrouter.ai/api/v1');
