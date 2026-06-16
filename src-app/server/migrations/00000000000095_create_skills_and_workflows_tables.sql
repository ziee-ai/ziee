-- Phase 8: skills + workflows as hub categories.
-- See plan §3 (consumer schema) + §4.0/§4.5 (workflow runner) + §4.6 (elicit) + §4.7 (artifacts).
--
-- Adds:
--   1. Extends hub_entities CHECK constraints (skill / workflow added — backward-compatible).
--   2. skills table — metadata only; bundle content lives on disk under <workspace>/<conv>/workflow/<run>/.
--   3. workflows table — same shape; compiled_ir_json reserved for §4.1 validator pattern (d).
--   4. group_skills + group_workflows — per-group access for system-scope items.
--   5. conversation_skill_overrides — per-conversation OPT-OUT (Path B progressive disclosure).
--   6. workflow_runs — execution audit + progress state.
--   7. Permission grants for Administrators (already covered by `*` wildcard, but explicit
--      mirrors the existing pattern at migration 85).

-- 1. Extend hub_entities CHECK constraints
ALTER TABLE hub_entities DROP CONSTRAINT valid_entity_type;
ALTER TABLE hub_entities ADD CONSTRAINT valid_entity_type
    CHECK (entity_type IN ('assistant', 'mcp_server', 'llm_model', 'skill', 'workflow'));
ALTER TABLE hub_entities DROP CONSTRAINT valid_hub_category;
ALTER TABLE hub_entities ADD CONSTRAINT valid_hub_category
    CHECK (hub_category IN ('assistant', 'mcp_server', 'model', 'skill', 'workflow'));

-- 2. skills — metadata only; content on disk at extracted_path
CREATE TABLE skills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,                                       -- reverse-DNS
    version TEXT,
    display_name TEXT,                                        -- from SKILL.md frontmatter `name`
    description TEXT,                                         -- from SKILL.md frontmatter `description`
    when_to_use TEXT,                                         -- from SKILL.md frontmatter `when_to_use`
    extracted_path TEXT NOT NULL,                             -- absolute path on disk
    bundle_sha256 TEXT NOT NULL,
    bundle_size_bytes BIGINT NOT NULL,
    file_count INTEGER NOT NULL,
    entry_point TEXT NOT NULL,                                -- "SKILL.md"
    frontmatter_json JSONB NOT NULL DEFAULT '{}'::jsonb,      -- FULL parsed frontmatter
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- 'built_in' = ziee's own capability/instruction skills, embedded in the
    -- binary and boot-synced (NOT hub-distributed, NOT uninstallable,
    -- version-locked to the binary). Like 'system', built-ins are
    -- unowned (owner_user_id IS NULL).
    scope VARCHAR(10) NOT NULL DEFAULT 'user'
        CHECK (scope IN ('user', 'system', 'built_in')),
    owner_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,  -- audit; separate from ownership
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_dev BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT skills_scope_owner_check CHECK (
        (scope = 'user' AND owner_user_id IS NOT NULL) OR
        (scope IN ('system', 'built_in') AND owner_user_id IS NULL)
    )
);
CREATE INDEX idx_skills_name ON skills(name);
CREATE INDEX idx_skills_enabled ON skills(enabled) WHERE enabled = TRUE;
CREATE INDEX idx_skills_owner ON skills(owner_user_id) WHERE scope = 'user';
-- Per-owner uniqueness (H1): the install keyspace is owner-scoped, NOT
-- global. One system copy per (name, version); one user copy per
-- (name, version, owner_user_id). Two partial unique indexes so the
-- NULL-distinct UNIQUE semantics don't accidentally let two system
-- rows share a (name, version) (NULL != NULL would permit dupes), while
-- the user index keys on the owner so user A and user B can each install
-- the same hub skill without colliding on each other's row or on-disk dir.
CREATE UNIQUE INDEX uniq_skills_system_name_version
    ON skills (name, version) WHERE scope = 'system';
CREATE UNIQUE INDEX uniq_skills_user_name_version_owner
    ON skills (name, version, owner_user_id) WHERE scope = 'user';
-- Built-in: exactly one row per name (the binary's embedded copy). The
-- boot sync upserts on this key so a binary upgrade replaces the row.
CREATE UNIQUE INDEX uniq_skills_builtin_name
    ON skills (name) WHERE scope = 'built_in';

-- 3. workflows — same metadata-only shape + scope/ownership pattern
CREATE TABLE workflows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    version TEXT,
    display_name TEXT,
    description TEXT,
    extracted_path TEXT NOT NULL,
    bundle_sha256 TEXT NOT NULL,
    bundle_size_bytes BIGINT NOT NULL,
    file_count INTEGER NOT NULL,
    entry_point TEXT NOT NULL,                                -- "workflow.yaml"
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    scope VARCHAR(10) NOT NULL DEFAULT 'user'
        CHECK (scope IN ('user', 'system')),
    owner_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_dev BOOLEAN NOT NULL DEFAULT FALSE,
    compiled_ir_json JSONB,                                   -- WorkflowIR (§4.1 pattern d); null until validator's compile pass runs
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workflows_scope_owner_check CHECK (
        (scope = 'user' AND owner_user_id IS NOT NULL) OR
        (scope = 'system' AND owner_user_id IS NULL)
    )
);
CREATE INDEX idx_workflows_name ON workflows(name);
CREATE INDEX idx_workflows_owner ON workflows(owner_user_id) WHERE scope = 'user';
-- Per-owner uniqueness (H1) — same rationale as skills above.
CREATE UNIQUE INDEX uniq_workflows_system_name_version
    ON workflows (name, version) WHERE scope = 'system';
CREATE UNIQUE INDEX uniq_workflows_user_name_version_owner
    ON workflows (name, version, owner_user_id) WHERE scope = 'user';

-- 4. Group assignments — restrict system-scope items to specific groups.
-- Empty (no rows for an item) = available to ALL users.
-- App-layer + trigger below: only scope='system' rows allowed here.
CREATE TABLE group_skills (
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (group_id, skill_id)
);
CREATE INDEX idx_group_skills_skill ON group_skills(skill_id);

CREATE TABLE group_workflows (
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (group_id, workflow_id)
);
CREATE INDEX idx_group_workflows_workflow ON group_workflows(workflow_id);

-- Trigger: reject group assignment to user-scope items.
CREATE OR REPLACE FUNCTION enforce_system_scope_for_group_skills() RETURNS trigger AS $$
BEGIN
    IF (SELECT scope FROM skills WHERE id = NEW.skill_id) <> 'system' THEN
        RAISE EXCEPTION 'group_skills: only system-scope skills can be assigned to groups (skill_id=%)', NEW.skill_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER group_skills_scope_check
    BEFORE INSERT OR UPDATE ON group_skills
    FOR EACH ROW EXECUTE FUNCTION enforce_system_scope_for_group_skills();

CREATE OR REPLACE FUNCTION enforce_system_scope_for_group_workflows() RETURNS trigger AS $$
BEGIN
    IF (SELECT scope FROM workflows WHERE id = NEW.workflow_id) <> 'system' THEN
        RAISE EXCEPTION 'group_workflows: only system-scope workflows can be assigned to groups (workflow_id=%)', NEW.workflow_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER group_workflows_scope_check
    BEFORE INSERT OR UPDATE ON group_workflows
    FOR EACH ROW EXECUTE FUNCTION enforce_system_scope_for_group_workflows();

-- 5. conversation_skill_overrides — per-conversation OPT-OUT (Path B)
-- Default: every installed/accessible skill is listed in the available-skills
-- prompt and reachable via skill_mcp. A row here means "hide this skill from
-- this conversation" (model never sees it in the listing).
CREATE TABLE conversation_skill_overrides (
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    hidden BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, skill_id)
);
CREATE INDEX idx_conversation_skill_overrides_conv ON conversation_skill_overrides(conversation_id);

-- 6. workflow_runs — execution audit + progress state
CREATE TABLE workflow_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL,  -- SNAPSHOT at run start
    sandbox_flavor TEXT,                                          -- from workflow.sandbox.flavor; null if no sandbox steps
    run_kind VARCHAR(10) NOT NULL DEFAULT 'normal'
        CHECK (run_kind IN ('normal', 'test', 'dry_run')),
    inputs_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    step_outputs_json JSONB NOT NULL DEFAULT '{}'::jsonb,         -- metadata only: {step_id: {path, size_bytes, sha256, preview, kind, parsed_as}}; content on disk under outputs/<step_id>
    step_item_progress_json JSONB NOT NULL DEFAULT '{}'::jsonb,   -- llm_map: {step_id: {completed, total, failed}}
    step_logs_json JSONB NOT NULL DEFAULT '{}'::jsonb,            -- {step_id: {prompt?, raw_output?, stderr?, items?, trace?}} per log: level
    step_artifacts_json JSONB NOT NULL DEFAULT '{}'::jsonb,       -- {step_id: [{path, size_bytes, sha256, mime_type, description, declared}]}
    pending_elicitation_json JSONB,                               -- {elicitation_id, step_id, message, schema, deadline_at} when an elicit step is awaiting user input
    final_output_json JSONB,                                      -- {name: {size_bytes, preview, expose, parsed_as}} — metadata for resolved outputs[]
    status VARCHAR(50) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    current_step TEXT,
    error_message TEXT,
    total_tokens BIGINT NOT NULL DEFAULT 0,  -- M4: BIGINT — a long run can exceed i32 range
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_workflow_runs_user ON workflow_runs(user_id);
CREATE INDEX idx_workflow_runs_status ON workflow_runs(status);
CREATE INDEX idx_workflow_runs_workflow ON workflow_runs(workflow_id);
CREATE INDEX idx_workflow_runs_conv ON workflow_runs(conversation_id) WHERE conversation_id IS NOT NULL;
CREATE INDEX idx_workflow_runs_run_kind ON workflow_runs(run_kind);

-- 7. Administrators wildcard already covers skills::* + workflows::*, but
-- explicit grants make the new permissions discoverable in the admin UI.
-- See migration 85 for the precedent (mcp::user_policy::edit).
-- These are app-layer permission registrations; the per-module
-- permissions.rs files declare the actual permission consts (registered
-- via the permission-registry macro, not a linkme slice).
