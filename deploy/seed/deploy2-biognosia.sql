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
--      is biognosia-only). The remaining built-in system servers (code_sandbox,
--      files, memory, …) are untouched.
--   4. Turns web fetch OFF deployment-wide — BOTH the visible `fetch` /
--      "Web Fetch" server AND the hidden web_search built-in that owns the
--      `web_search` + `fetch_url` tools.
--
-- NOTE: LLM providers/models are NOT seeded here — deploy2 gets its LLM config
-- copied from the live :8080 instance (local-provider.sql is dropped from the
-- ziee-seed command for deploy2 to avoid a duplicate provider).

\set ON_ERROR_STOP on

BEGIN;

-- ── 1. Give Users the user-level read-only assistant permission ───────────────
-- `assistants::read` gates the chat-input assistant selector (a user reading their
-- OWN assistants). It is NOT `assistant_templates::read` — that is the admin-only
-- system-wide-template management view and must not be granted to regular Users.
-- Idempotent; runs AFTER seed.sql's step-4 permission reduction so it isn't stripped.
UPDATE groups
   SET permissions = array_remove(permissions, 'assistant_templates::read'),
       updated_at = NOW()
 WHERE name = 'Users' AND is_system = true AND is_default = true
   AND 'assistant_templates::read' = ANY(permissions);
UPDATE groups
   SET permissions = array_append(permissions, 'assistants::read'),
       updated_at = NOW()
 WHERE name = 'Users' AND is_system = true AND is_default = true
   AND NOT ('assistants::read' = ANY(permissions));

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

-- ── 2b. Clone BioGnosia to every EXISTING user as their default ───────────────
-- CloneTemplateAssistantsHandler only clones the default template on NEW signup,
-- so users who already existed when this deploy runs would never get it. Give
-- every current user a personal BioGnosia (idempotent — skipped for anyone who
-- already has one). Single-purpose deploy: BioGnosia is everyone's default, so
-- first clear any OTHER personal default, then ensure every BioGnosia clone is
-- default.
UPDATE assistants
   SET is_default = false, updated_at = NOW()
 WHERE is_template = false AND created_by IS NOT NULL
   AND is_default = true AND name <> 'BioGnosia';

INSERT INTO assistants (name, description, instructions, parameters,
        created_by, is_template, is_default, enabled)
SELECT t.name, t.description, t.instructions, t.parameters,
       u.id, false, true, true
FROM assistants t CROSS JOIN users u
WHERE t.name = 'BioGnosia' AND t.is_template = true
  AND NOT EXISTS (
      SELECT 1 FROM assistants a
       WHERE a.created_by = u.id AND a.name = 'BioGnosia' AND a.is_template = false
  );

UPDATE assistants
   SET is_default = true, updated_at = NOW()
 WHERE is_template = false AND created_by IS NOT NULL
   AND name = 'BioGnosia' AND is_default = false;

-- ── 3. biognosia as the ONLY external data MCP server ─────────────────────────
-- (Re)assert the biognosia system server (is_system=true → user_id=NULL, per
-- CHECK system_server_must_have_no_owner). Reachable via the host gateway
-- (extra_hosts host.docker.internal:host-gateway) at the host-published :8081.
-- biognosia MCP URL: overridable with `-v biognosia_url='…'` (the deployed server
-- runs biognosia at a different address than the local :8081). Defaults to the
-- local-test URL when not supplied.
\if :{?biognosia_url}
\else
\set biognosia_url 'http://host.docker.internal:8081/mcp'
\endif
-- Keyed on (name, is_system): create only when absent, then enforce fields.
INSERT INTO mcp_servers (id, user_id, name, display_name, description,
        enabled, is_system, is_built_in, transport_type, url, usage_mode,
        supports_sampling, timeout_seconds)
SELECT gen_random_uuid(), NULL, 'biognosia', 'Biognosia RAG',
        'Biomedical RAG over lightrag DBs',
        true, true, false, 'http', :'biognosia_url', 'auto', true, 300
WHERE NOT EXISTS (SELECT 1 FROM mcp_servers WHERE name = 'biognosia' AND is_system);

UPDATE mcp_servers
   SET url = :'biognosia_url', enabled = true, usage_mode = 'auto',
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

-- ...and to Administrators. MCP server ACCESS is by GROUP MEMBERSHIP, not by
-- permission — the `*` wildcard does NOT make a system server accessible. An
-- admin who is only in Administrators (the state seed.sql step 5 creates) would
-- otherwise get NO biognosia MCP tag on the composer and NO "MCP tools &
-- servers" entry in the + menu, because the server never enters their
-- accessible-server list. Mirrors the Users grant above; additive + idempotent.
INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
SELECT g.id, s.id
  FROM groups g, mcp_servers s
 WHERE g.name = 'Administrators' AND s.is_system AND s.name = 'biognosia'
ON CONFLICT (group_id, mcp_server_id) DO NOTHING;

-- deploy2 is biognosia-only: drop the rcpa/dscc system servers that seed.sql
-- registers (their group assignments cascade away via FK ON DELETE CASCADE).
DELETE FROM mcp_servers WHERE is_system = true AND name IN ('rcpa', 'dscc');

-- ── 4. Disable web fetch, deployment-wide ────────────────────────────────────
-- "webfetch" is TWO distinct surfaces in this codebase; both are turned off.
-- Idempotent: the trailing `AND enabled` makes a re-run a no-op. biognosia is
-- unaffected (it is neither of these rows).
--
-- (a) The `fetch` / "Web Fetch" stdio server (`uvx mcp-server-fetch`, fixed id
--     865f06fa-c4e5-4eb3-9801-5804f67062c2), seeded by the mcp seed migration
--     and assigned to the Users group. This is the row users SEE on the System
--     MCP page. It is migration-seeded and never boot-upserted, so once
--     disabled it stays disabled across restarts.
UPDATE mcp_servers
   SET enabled = false, updated_at = NOW()
 WHERE is_system = true AND name = 'fetch' AND enabled;

-- (b) The hidden `web_search` built-in, which owns BOTH the `web_search` and
--     `fetch_url` tools. `attach_gate_open()` is
--     `settings.enabled && any_configured_in_chain(...)`, so clearing the
--     singleton's `enabled` closes the attach gate for both tools and also
--     drops the model-facing WEB_SEARCH_NUDGE prompt text. (The server row
--     itself is re-upserted at every boot, but that upsert re-asserts only the
--     identity columns — never `enabled` — so the gate below is the durable
--     switch.)
UPDATE web_search_settings
   SET enabled = false, updated_at = NOW()
 WHERE id = true AND enabled;

COMMIT;

\echo 'deploy2 BioGnosia customization seed applied successfully.'
