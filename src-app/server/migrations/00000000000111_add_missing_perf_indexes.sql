-- Performance indexes for hot list/filter queries that previously did
-- sequential scans (audit: medium-severity missing-index findings).

-- list_user_workflows orders a user's runs by recency; the existing
-- (workflow_id, user_id, created_at) index is not usable for a user-only
-- filter because workflow_id leads.
CREATE INDEX IF NOT EXISTS idx_workflow_runs_user_created
    ON workflow_runs (user_id, created_at DESC);

-- fail_orphaned_runs (+ retention/prune) filter workflow_runs by created_at.
CREATE INDEX IF NOT EXISTS idx_workflow_runs_created_at
    ON workflow_runs (created_at);

-- mcp_servers queries filter on is_built_in to separate built-in vs user rows.
CREATE INDEX IF NOT EXISTS idx_mcp_servers_is_built_in
    ON mcp_servers (is_built_in);

-- user list endpoints order by created_at.
CREATE INDEX IF NOT EXISTS idx_users_created_at
    ON users (created_at);

-- find_user_by_email_for_linking matches on LOWER(email); a plain email
-- index can't serve that, so add a functional index on LOWER(email).
CREATE INDEX IF NOT EXISTS idx_users_lower_email
    ON users (LOWER(email));
