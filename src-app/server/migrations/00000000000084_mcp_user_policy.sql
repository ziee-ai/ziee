-- Admin-controlled policy gating what kinds of MCP servers regular
-- users can install, and what sandbox flavor user-installed stdio MCP
-- servers must run inside.
--
-- Default policy: HTTP + STDIO with the 'full' sandbox flavor.
-- Preserves the current "users can add anything" UX but wraps every
-- stdio MCP in bwrap isolation. Admins can tighten on the System MCP
-- page (PUT /api/mcp/user-policy).
--
-- Semantics:
--   allowed_transports == []         → users cannot add any MCP server
--                                       AND the MCP tab is hidden in
--                                       the Hub for non-admin users.
--   'stdio' in allowed_transports    → user_stdio_sandbox_flavor MUST
--                                       be set (validated at PUT time).
--                                       User-create handlers force-set
--                                       run_in_sandbox=true +
--                                       sandbox_flavor=<this> on every
--                                       user-owned stdio row,
--                                       regardless of what the client
--                                       sent.
--
-- The CHECK on id ensures singleton-row semantics (only id=1 exists).
--
-- NOTE: the `mcp_servers.sandbox_flavor` column was already added
-- by migration 83 (parallel sandbox-mcp-flavor feature on main):
-- `VARCHAR(32) NOT NULL DEFAULT 'full'`. Migration 84 does NOT add
-- the column — it only adds the policy table and force-migrates
-- existing user stdio rows into sandbox mode.

CREATE TABLE mcp_user_policy (
    id INT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    allowed_transports TEXT[] NOT NULL DEFAULT ARRAY['http', 'stdio']::TEXT[],
    user_stdio_sandbox_flavor TEXT DEFAULT 'full',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO mcp_user_policy (id) VALUES (1) ON CONFLICT DO NOTHING;

-- One-shot upgrade migration: every existing user-owned stdio server
-- gets force-flipped into sandbox mode with the default policy flavor.
-- Servers that depended on host-process semantics (filesystem access,
-- host network, host env, host binaries beyond what the rootfs ships)
-- will start failing the connection-health probe after upgrade with a
-- clear "sandbox required by policy" reason. Users re-add as HTTP, or
-- the admin promotes the server to system + opts out of run_in_sandbox.
--
-- This UPDATE overwrites migration 83's preservation pass for the
-- user-owned subset: migration 83 set `sandbox_flavor='minimal'` on
-- ALL rows where `run_in_sandbox=true` (preserving the legacy hardcoded
-- minimal-only behavior); here we promote user-owned stdio rows to
-- `'full'` because the default user policy requires the full flavor.
-- System rows are untouched by this UPDATE — they keep whatever
-- migration 83 chose ('minimal' if they were sandboxed before, or
-- the column default 'full' if they're new).
UPDATE mcp_servers
SET run_in_sandbox = TRUE,
    sandbox_flavor = 'full'
WHERE is_system = FALSE
  AND transport_type = 'stdio';
