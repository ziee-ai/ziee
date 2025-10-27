-- ===============================
-- Authentication and User Management System
-- ===============================

-- ===============================
-- 1. UTILITY FUNCTIONS
-- ===============================

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- ===============================
-- 2. CORE USER SYSTEM
-- ===============================

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    profile JSONB,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_protected BOOLEAN NOT NULL DEFAULT FALSE,
    last_login_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- User emails table
CREATE TABLE user_emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    address VARCHAR(255) NOT NULL UNIQUE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- User services table (for password auth and other services)
CREATE TABLE user_services (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    service_name VARCHAR(50) NOT NULL,
    service_data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    UNIQUE(user_id, service_name)
);

-- User login tokens table (for resume tokens)
CREATE TABLE user_login_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) NOT NULL UNIQUE,
    when_created BIGINT NOT NULL, -- Unix timestamp in milliseconds
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- User settings table
CREATE TABLE user_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key VARCHAR(255) NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    UNIQUE(user_id, key)
);

-- ===============================
-- 3. AUTHENTICATION PROVIDERS
-- ===============================

-- Auth providers table (local, LDAP, OAuth, OIDC, SAML)
CREATE TABLE auth_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    provider_type VARCHAR(50) NOT NULL CHECK (provider_type IN ('local', 'ldap', 'oauth2', 'oidc', 'saml')),
    enabled BOOLEAN DEFAULT TRUE NOT NULL,
    priority INTEGER DEFAULT 0 NOT NULL,
    config JSONB DEFAULT '{}' NOT NULL,
    mapping_rules JSONB DEFAULT '{}' NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- User auth links table (links users to external auth providers)
CREATE TABLE user_auth_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id UUID NOT NULL REFERENCES auth_providers(id) ON DELETE CASCADE,
    external_id VARCHAR(512) NOT NULL,
    external_username VARCHAR(255),
    external_email VARCHAR(255),
    external_metadata JSONB DEFAULT '{}',
    last_login_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    UNIQUE(provider_id, external_id)
);

-- Auth sessions table (for OAuth/SAML flows)
CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key VARCHAR(255) NOT NULL UNIQUE,
    provider_id UUID NOT NULL REFERENCES auth_providers(id) ON DELETE CASCADE,
    state VARCHAR(512) NOT NULL,
    nonce VARCHAR(512),
    code_verifier VARCHAR(512),
    redirect_uri VARCHAR(512),
    metadata JSONB DEFAULT '{}',
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- ===============================
-- 4. INDEXES
-- ===============================

-- Users indexes
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_created_at ON users(created_at);
CREATE INDEX idx_users_profile ON users USING GIN(profile);
CREATE INDEX idx_users_is_active ON users(is_active);
CREATE INDEX idx_users_is_protected ON users(is_protected);
CREATE INDEX idx_users_last_login_at ON users(last_login_at);
CREATE INDEX idx_users_updated_at ON users(updated_at);

-- User emails indexes
CREATE INDEX idx_user_emails_user_id ON user_emails(user_id);
CREATE INDEX idx_user_emails_address ON user_emails(address);
CREATE INDEX idx_user_emails_verified ON user_emails(verified);

-- User services indexes
CREATE INDEX idx_user_services_user_id ON user_services(user_id);
CREATE INDEX idx_user_services_service_name ON user_services(service_name);
CREATE INDEX idx_user_services_data ON user_services USING GIN(service_data);

-- User login tokens indexes
CREATE INDEX idx_user_login_tokens_user_id ON user_login_tokens(user_id);
CREATE INDEX idx_user_login_tokens_token ON user_login_tokens(token);
CREATE INDEX idx_user_login_tokens_expires_at ON user_login_tokens(expires_at);

-- User settings indexes
CREATE INDEX idx_user_settings_user_id ON user_settings(user_id);
CREATE INDEX idx_user_settings_key ON user_settings(key);
CREATE INDEX idx_user_settings_user_id_key ON user_settings(user_id, key);
CREATE INDEX idx_user_settings_value ON user_settings USING GIN(value);

-- Auth providers indexes
CREATE INDEX idx_auth_providers_type ON auth_providers(provider_type);
CREATE INDEX idx_auth_providers_enabled ON auth_providers(enabled);
CREATE INDEX idx_auth_providers_priority ON auth_providers(priority DESC);

-- Ensure only one local provider can exist
CREATE UNIQUE INDEX idx_auth_providers_unique_local
ON auth_providers(provider_type)
WHERE provider_type = 'local';

-- User auth links indexes
CREATE INDEX idx_user_auth_links_user ON user_auth_links(user_id);
CREATE INDEX idx_user_auth_links_provider ON user_auth_links(provider_id);
CREATE INDEX idx_user_auth_links_external_id ON user_auth_links(external_id);

-- Auth sessions indexes
CREATE INDEX idx_auth_sessions_session_key ON auth_sessions(session_key);
CREATE INDEX idx_auth_sessions_provider ON auth_sessions(provider_id);
CREATE INDEX idx_auth_sessions_expires ON auth_sessions(expires_at);

-- ===============================
-- 5. TRIGGERS
-- ===============================

-- Users updated_at trigger
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- User settings updated_at trigger
CREATE TRIGGER update_user_settings_updated_at
    BEFORE UPDATE ON user_settings
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Auth providers updated_at trigger
CREATE TRIGGER update_auth_providers_updated_at
    BEFORE UPDATE ON auth_providers
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- User auth links updated_at trigger
CREATE TRIGGER update_user_auth_links_updated_at
    BEFORE UPDATE ON user_auth_links
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ===============================
-- 6. DEFAULT DATA
-- ===============================

-- Insert default local auth provider
INSERT INTO auth_providers (name, provider_type, enabled, priority, config, mapping_rules)
VALUES (
    'Local Authentication',
    'local',
    TRUE,
    100,
    '{}',
    '{}'
);
