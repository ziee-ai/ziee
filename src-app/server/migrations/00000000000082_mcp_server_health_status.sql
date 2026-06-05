-- Persistent connection-health record for MCP servers.
--
-- Backstops the in-memory `connection_health::probe` outcome with a
-- DB column the UI can read directly — admins / users can see WHY a
-- server is disabled without having to dig through server logs or
-- re-run the probe.
--
-- Recorded at four points (see `mcp::connection_health`):
--   1. Boot-time startup health check
--   2. Create-flow probe (`enforce_on_create`)
--   3. Update-flow enable-transition probe
--   4. Explicit "Test Connection" button
--
-- Columns:
--   last_health_check_at      — when the probe last ran. NULL on
--                                fresh rows that have never been probed
--                                (hub installs that default to
--                                disabled, manual creates that started
--                                disabled).
--   last_health_check_status  — 'untested' | 'healthy' | 'unhealthy'.
--                                Mirrors the at-column's nullability:
--                                'untested' when at IS NULL.
--   last_health_check_reason  — Human reason on failure (verbatim from
--                                `TestMcpConnectionResponse.message`).
--                                NULL on healthy / untested.

ALTER TABLE mcp_servers
    ADD COLUMN last_health_check_at TIMESTAMPTZ,
    ADD COLUMN last_health_check_status TEXT NOT NULL DEFAULT 'untested'
        CHECK (last_health_check_status IN ('untested', 'healthy', 'unhealthy')),
    ADD COLUMN last_health_check_reason TEXT;

CREATE INDEX idx_mcp_servers_health_status_unhealthy
    ON mcp_servers (last_health_check_status)
    WHERE last_health_check_status = 'unhealthy';
