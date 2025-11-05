-- Create LLM model repositories table (Hugging Face, GitHub, custom sources)
-- Copied from react-test repositories table and refactored for ziee-chat

CREATE TABLE llm_repositories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    url VARCHAR(512) NOT NULL,
    auth_type VARCHAR(50) NOT NULL CHECK (auth_type IN ('none', 'api_key', 'basic_auth', 'bearer_token')),
    auth_config JSONB DEFAULT '{}',
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    built_in BOOLEAN DEFAULT FALSE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    UNIQUE(name)
);

-- Create trigger for automatic updated_at timestamp
CREATE TRIGGER update_llm_repositories_updated_at
    BEFORE UPDATE ON llm_repositories
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert default built-in repositories
INSERT INTO llm_repositories (name, url, auth_type, auth_config, enabled, built_in) VALUES
    ('Hugging Face Hub', 'https://huggingface.co', 'api_key', '{"api_key": "", "auth_test_api_endpoint": "https://huggingface.co/api/whoami-v2"}', true, true),
    ('GitHub', 'https://github.com', 'bearer_token', '{"token": "", "auth_test_api_endpoint": "https://api.github.com/user"}', true, true);

-- Add table and column comments for documentation
COMMENT ON TABLE llm_repositories IS 'LLM model repositories (Hugging Face, GitHub, custom sources)';
COMMENT ON COLUMN llm_repositories.auth_type IS 'Authentication type: none, api_key, basic_auth, bearer_token';
COMMENT ON COLUMN llm_repositories.auth_config IS 'JSON object containing auth credentials and optional test endpoint';
COMMENT ON COLUMN llm_repositories.built_in IS 'true for default repositories (Hugging Face, GitHub) - cannot be deleted';
COMMENT ON COLUMN llm_repositories.enabled IS 'Whether this repository is currently enabled for use';
