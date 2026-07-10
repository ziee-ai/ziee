-- Unattended tool-call policy for scheduled tasks (DEC-17 "safe default + per-task
-- allow-list"). Two additive JSONB columns; existing rows default to the safe floor
-- (empty allow-list ⇒ only built-in read-only tools run unattended).

-- Per-task allow-list: MCP servers/tools the creator pre-authorizes to run in an
-- UNATTENDED (scheduled) firing without the interactive per-call approval speed-bump.
-- Shape: JSON array of { "server_id": "<uuid>", "tool_name": "<name>"? } — tool_name
-- omitted/null ⇒ the whole server is allow-listed. Validated at create/update to be a
-- subset of the user's currently-accessible servers (never widens access).
ALTER TABLE scheduled_tasks
    ADD COLUMN allowed_unattended_tools JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN scheduled_tasks.allowed_unattended_tools IS
    'Per-task allow-list of MCP servers/tools that may run unattended without per-call approval (DEC-17); subset of the owner''s accessible servers. Empty = built-in read-only tools only.';

-- Honest reporting: which tools a firing SKIPPED because they were not permitted
-- unattended (approval-required + not allow-listed). Shape: JSON array of
-- { "tool_name": "<name>", "reason": "<why>" }. Surfaced in the run history + the
-- completion notification so a truncated result is never reported as a clean success.
ALTER TABLE scheduled_task_runs
    ADD COLUMN skipped_tools JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN scheduled_task_runs.skipped_tools IS
    'Tools skipped during this firing because they were not permitted unattended (DEC-17.5); [] when none.';
