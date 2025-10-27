-- ===============================
-- User Group and Membership System
-- ===============================

-- ===============================
-- 1. USER GROUP TABLES
-- ===============================

-- User groups table for role-based access control
CREATE TABLE user_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    permissions JSONB DEFAULT '[]' NOT NULL, -- Array format for AWS-style permissions
    is_protected BOOLEAN DEFAULT FALSE NOT NULL,
    is_active BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- User group memberships table (many-to-many relationship)
CREATE TABLE user_group_memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES user_groups(id) ON DELETE CASCADE,
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    assigned_by UUID REFERENCES users(id),
    UNIQUE(user_id, group_id)
);

-- ===============================
-- 2. INDEXES
-- ===============================

-- User groups indexes
CREATE INDEX idx_user_groups_name ON user_groups(name);
CREATE INDEX idx_user_groups_is_protected ON user_groups(is_protected);
CREATE INDEX idx_user_groups_is_active ON user_groups(is_active);
CREATE INDEX idx_user_groups_permissions ON user_groups USING GIN(permissions);

-- User group memberships indexes
CREATE INDEX idx_user_group_memberships_user_id ON user_group_memberships(user_id);
CREATE INDEX idx_user_group_memberships_group_id ON user_group_memberships(group_id);
CREATE INDEX idx_user_group_memberships_assigned_by ON user_group_memberships(assigned_by);

-- ===============================
-- 3. TRIGGERS
-- ===============================

-- User groups updated_at trigger
CREATE TRIGGER update_user_groups_updated_at
    BEFORE UPDATE ON user_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ===============================
-- 4. DEFAULT DATA
-- ===============================

-- Create default admin group with full permissions
INSERT INTO user_groups (name, description, permissions, is_protected, is_active)
VALUES (
    'admin',
    'Administrator group with full permissions',
    '["*"]',
    TRUE,
    TRUE
);

-- Create default user group with basic permissions
INSERT INTO user_groups (name, description, permissions, is_protected, is_active)
VALUES (
    'user',
    'Default user group with basic permissions',
    '["chat::use", "profile::edit", "settings::read", "settings::edit", "settings::delete"]',
    TRUE,
    TRUE
);
