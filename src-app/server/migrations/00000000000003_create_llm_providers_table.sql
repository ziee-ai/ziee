-- LLM Providers table (for OpenAI, Anthropic, Local, Groq, etc.)
CREATE TABLE llm_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    provider_type VARCHAR(50) NOT NULL CHECK (
        provider_type IN ('local', 'openai', 'anthropic', 'groq', 'gemini', 'mistral', 'deepseek', 'huggingface', 'custom')
    ),
    enabled BOOLEAN DEFAULT FALSE NOT NULL,
    api_key TEXT,
    base_url VARCHAR(512),
    built_in BOOLEAN DEFAULT FALSE NOT NULL,
    proxy_settings JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Indexes
CREATE INDEX idx_llm_providers_type ON llm_providers(provider_type);
CREATE INDEX idx_llm_providers_enabled ON llm_providers(enabled);

-- User group to LLM provider assignments
CREATE TABLE user_group_llm_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES llm_providers(id) ON DELETE CASCADE,
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    UNIQUE(group_id, provider_id)
);

CREATE INDEX idx_ugp_group ON user_group_llm_providers(group_id);
CREATE INDEX idx_ugp_provider ON user_group_llm_providers(provider_id);

-- Trigger for updated_at
CREATE TRIGGER update_llm_providers_updated_at
    BEFORE UPDATE ON llm_providers
    FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Insert built-in providers with base URLs
INSERT INTO llm_providers (name, provider_type, enabled, built_in, base_url) VALUES
    ('OpenAI', 'openai', false, true, 'https://api.openai.com/v1'),
    ('Anthropic', 'anthropic', false, true, 'https://api.anthropic.com/v1'),
    ('Groq', 'groq', false, true, 'https://api.groq.com/openai/v1'),
    ('Google Gemini', 'gemini', false, true, 'https://generativelanguage.googleapis.com/v1beta'),
    ('Mistral AI', 'mistral', false, true, 'https://api.mistral.ai/v1'),
    ('DeepSeek', 'deepseek', false, true, 'https://api.deepseek.com'),
    ('Local', 'local', false, true, 'http://localhost:8080/v1');
