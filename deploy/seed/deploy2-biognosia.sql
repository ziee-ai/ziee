-- deploy2 "BioGnosia" customization seed — runs AFTER seed.sql (same psql
-- invocation, shares the -v vars incl. :storage_key). Idempotent: safe to
-- re-run on every deploy.
--
-- WHAT IT DOES:
--   1. Grants the default Users group the read-only assistant-template
--      permission (so users can see the BioGnosia template in the UI).
--   2. Installs a default BioGnosia system template that auto-clones to every
--      new user (unsets any other default template first, so ONLY BioGnosia
--      clones).
--   3. Makes biognosia the ONLY external data (HTTP) MCP server: (re)asserts the
--      biognosia system server at the local host gateway, assigns it to Users,
--      and removes the rcpa/dscc system servers that seed.sql registers (deploy2
--      is biognosia-only). Built-in system servers (code_sandbox, files, memory,
--      fetch, web_search, …) are untouched.
--
-- NOTE: LLM providers/models are NOT seeded here — deploy2 gets its LLM config
-- copied from the live :8080 instance (local-provider.sql is dropped from the
-- ziee-seed command for deploy2 to avoid a duplicate provider).

\set ON_ERROR_STOP on

BEGIN;

-- ── 1. Grant Users the read-only assistant-template permission ────────────────
-- Mirrors migration 00000000000061's idempotent array_append pattern. Runs AFTER
-- seed.sql's step-4 permission reduction, so this grant is not stripped.
UPDATE groups
   SET permissions = array_append(permissions, 'assistant_templates::read'),
       updated_at = NOW()
 WHERE name = 'Users' AND is_system = true AND is_default = true
   AND NOT ('assistant_templates::read' = ANY(permissions));

-- ── 2. BioGnosia default system template ──────────────────────────────────────
-- A system template = is_template=true + created_by=NULL (CHECK
-- template_must_have_no_owner). is_default=true + enabled=true makes
-- CloneTemplateAssistantsHandler clone it to every new user. First unset any
-- OTHER default template so ONLY BioGnosia is cloned for new users.
UPDATE assistants
   SET is_default = false, updated_at = NOW()
 WHERE is_template = true AND is_default = true AND name <> 'BioGnosia';

INSERT INTO assistants (name, description, instructions, parameters,
        created_by, is_template, is_default, enabled)
SELECT 'BioGnosia',
       'Biology assistant with access to the Biognosia biomedical RAG knowledge base.',
       'You are BioGnosia, a helpful assistant for biological questions. When a user asks a biological question, carefully decide whether it should be routed to the Biognosia tool (a biomedical RAG knowledge base) and use it when appropriate. For non-biological questions, answer normally.',
       '{"temperature": 0.7, "max_tokens": 2048, "top_p": 0.9}'::jsonb,
       NULL, true, true, true
WHERE NOT EXISTS (
    SELECT 1 FROM assistants WHERE name = 'BioGnosia' AND is_template = true
);

-- Re-assert the declared fields on an existing BioGnosia template (idempotent
-- enforce, mirrors seed.sql's server upserts).
UPDATE assistants
   SET is_template = true, created_by = NULL, is_default = true, enabled = true,
       description = 'Biology assistant with access to the Biognosia biomedical RAG knowledge base.',
       instructions = 'You are BioGnosia, a helpful assistant for biological questions. When a user asks a biological question, carefully decide whether it should be routed to the Biognosia tool (a biomedical RAG knowledge base) and use it when appropriate. For non-biological questions, answer normally.',
       updated_at = NOW()
 WHERE name = 'BioGnosia' AND is_template = true;

-- ── 3. biognosia as the ONLY external data MCP server ─────────────────────────
-- (Re)assert the biognosia system server (is_system=true → user_id=NULL, per
-- CHECK system_server_must_have_no_owner). Reachable via the host gateway
-- (extra_hosts host.docker.internal:host-gateway) at the host-published :8081.
-- Keyed on (name, is_system): create only when absent, then enforce fields.
INSERT INTO mcp_servers (id, user_id, name, display_name, description,
        enabled, is_system, is_built_in, transport_type, url, usage_mode,
        supports_sampling, timeout_seconds)
SELECT gen_random_uuid(), NULL, 'biognosia', 'Biognosia RAG',
        'Biomedical RAG over lightrag DBs',
        true, true, false, 'http', 'http://host.docker.internal:8081/mcp', 'auto', true, 300
WHERE NOT EXISTS (SELECT 1 FROM mcp_servers WHERE name = 'biognosia' AND is_system);

UPDATE mcp_servers
   SET url = 'http://host.docker.internal:8081/mcp', enabled = true, usage_mode = 'auto',
       display_name = 'Biognosia RAG', description = 'Biomedical RAG over lightrag DBs',
       supports_sampling = true, transport_type = 'http', timeout_seconds = 300,
       is_built_in = false, updated_at = NOW()
 WHERE name = 'biognosia' AND is_system;

-- Assign biognosia to the default Users group (system server in no group is
-- unusable by non-admins). Additive + idempotent.
INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
SELECT g.id, s.id
  FROM groups g, mcp_servers s
 WHERE g.name = 'Users' AND s.is_system AND s.name = 'biognosia'
ON CONFLICT (group_id, mcp_server_id) DO NOTHING;

-- deploy2 is biognosia-only: drop the rcpa/dscc system servers that seed.sql
-- registers (their group assignments cascade away via FK ON DELETE CASCADE).
DELETE FROM mcp_servers WHERE is_system = true AND name IN ('rcpa', 'dscc');

COMMIT;

\echo 'deploy2 BioGnosia customization seed applied successfully.'
