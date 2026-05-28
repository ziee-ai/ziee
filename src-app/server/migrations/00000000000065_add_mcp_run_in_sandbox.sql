-- Per-server opt-in to run an admin/system stdio MCP server inside the
-- bwrap-isolated code_sandbox. Default off — behavior unchanged for
-- everything that doesn't flip it.
--
-- Only honored at spawn time when (is_system AND transport_type='stdio').
-- For other combinations the column persists but the spawn path ignores
-- it; the UI hides the toggle except in admin-system + stdio mode.
ALTER TABLE mcp_servers
    ADD COLUMN run_in_sandbox BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN mcp_servers.run_in_sandbox IS
    'When true AND is_system AND transport_type=''stdio'', launch the MCP subprocess inside the code_sandbox bwrap isolation. See server/src/modules/mcp/client/stdio.rs.';
