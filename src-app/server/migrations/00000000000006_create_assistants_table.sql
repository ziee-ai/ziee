-- ===============================
-- ASSISTANTS MANAGEMENT
-- ===============================

-- Create assistants table
-- Assistants define AI behavior with instructions, parameters, and settings
-- Supports both user-created assistants and system-wide templates
CREATE TABLE assistants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    instructions TEXT,
    parameters JSONB DEFAULT '{}',
    created_by UUID REFERENCES users(id) ON DELETE CASCADE,
    is_template BOOLEAN DEFAULT false NOT NULL,
    is_default BOOLEAN DEFAULT false NOT NULL,
    enabled BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,

    -- Constraints
    CONSTRAINT unique_user_assistant_name UNIQUE (name, created_by),
    CONSTRAINT template_must_have_no_owner CHECK (
        (is_template = true AND created_by IS NULL) OR
        (is_template = false)
    )
);

-- Indexes for performance
CREATE INDEX idx_assistants_created_by ON assistants(created_by);
CREATE INDEX idx_assistants_is_template ON assistants(is_template);
CREATE INDEX idx_assistants_is_default ON assistants(is_default);
CREATE INDEX idx_assistants_enabled ON assistants(enabled);
CREATE INDEX idx_assistants_name ON assistants(name);

-- Trigger to update updated_at timestamp
CREATE TRIGGER update_assistants_updated_at
    BEFORE UPDATE ON assistants
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Comments
COMMENT ON TABLE assistants IS 'Assistants with user-created and system template configurations';
COMMENT ON COLUMN assistants.name IS 'Unique name for the assistant within user scope';
COMMENT ON COLUMN assistants.description IS 'Brief description of the assistant purpose';
COMMENT ON COLUMN assistants.instructions IS 'System instructions for the AI assistant';
COMMENT ON COLUMN assistants.parameters IS 'Model parameters (temperature, max_tokens, etc.) as JSONB';
COMMENT ON COLUMN assistants.created_by IS 'User who created this assistant (NULL for templates)';
COMMENT ON COLUMN assistants.is_template IS 'Whether this is a system-wide template available to all users';
COMMENT ON COLUMN assistants.is_default IS 'Whether this is the default assistant for the user/template context';
COMMENT ON COLUMN assistants.enabled IS 'Whether this assistant is enabled (false means disabled/soft-deleted)';
