-- Per-server sandbox rootfs flavor for MCP servers launched inside the
-- code_sandbox (run_in_sandbox = true). Defaults to 'full' because the
-- full flavor ships the runtimes real MCP servers need (Node 24 + npm
-- → npx, uv → uvx, python3, R); 'minimal' is python3-only.
--
-- Only honored at spawn time when (is_system AND transport_type='stdio'
-- AND run_in_sandbox). Ignored otherwise (the host path resolves
-- commands against the bundled bun/uv instead).
ALTER TABLE mcp_servers
    ADD COLUMN sandbox_flavor VARCHAR(32) NOT NULL DEFAULT 'full';

-- Preserve behavior for servers that were ALREADY sandboxed before this
-- column existed: they ran on the old hardcoded 'minimal' flavor. Pin
-- them to 'minimal' so the next connect doesn't force an ~850 MB 'full'
-- download. New servers still default to 'full' (the column default).
UPDATE mcp_servers SET sandbox_flavor = 'minimal' WHERE run_in_sandbox = true;

COMMENT ON COLUMN mcp_servers.sandbox_flavor IS
    'Rootfs flavor (KNOWN_FLAVORS, e.g. minimal/full) used when run_in_sandbox launches this stdio server inside the code_sandbox. Defaults to full. See server/src/modules/code_sandbox/mcp_spawn.rs.';
