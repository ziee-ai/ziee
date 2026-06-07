-- Persistent connection-health record for LLM repositories.
--
-- Mirrors `mcp_servers` migration 82 for the `llm_repositories` table.
-- Today the UI flips a repository's `enabled` switch with no
-- verification that the upstream is actually reachable — a bad/missing
-- token leaves the row "enabled" forever, surfacing as confused
-- download failures later. These columns backstop a new
-- `connection_health` module that probes on save / enable-transition
-- and at boot, persisting WHY a repo got auto-disabled so the UI can
-- render an Alert without digging through server logs.
--
-- Recorded at four points (see `llm_repository::connection_health`):
--   1. Boot-time startup health check (auto-disables failing repos)
--   2. Create-flow probe (`enforce_on_create`)
--   3. Update-flow enable-transition probe (`enforce_on_update_transition`)
--   4. The existing form-based "Test Connection" path in the drawer
--      (kept untouched; it still validates before save)
--
-- Columns mirror migration 82 verbatim — same shape, same defaults,
-- same CHECK constraint, same partial-index strategy on the unhealthy
-- slice (the UI query that drives the Alert badge benefits from the
-- partial index even on tables with very few rows, because it lets
-- Postgres skip the `last_health_check_status = 'unhealthy'`
-- comparison entirely).
--
-- No data migration. The product decision was explicitly NOT to
-- force-flip existing `enabled = TRUE` rows for built-in HuggingFace
-- / GitHub: admins with working credentials see no change. Fresh
-- installs: the seed rows insert as enabled, then the boot probe
-- (added in this same change set) finds missing credentials and
-- auto-disables them — same end state without disrupting working
-- installs.

ALTER TABLE llm_repositories
    ADD COLUMN last_health_check_at TIMESTAMPTZ,
    ADD COLUMN last_health_check_status TEXT NOT NULL DEFAULT 'untested'
        CHECK (last_health_check_status IN ('untested', 'healthy', 'unhealthy')),
    ADD COLUMN last_health_check_reason TEXT;

CREATE INDEX idx_llm_repositories_health_status_unhealthy
    ON llm_repositories (last_health_check_status)
    WHERE last_health_check_status = 'unhealthy';
