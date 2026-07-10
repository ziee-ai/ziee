-- Per-firing audit history for scheduled tasks (the "Runs" / activity feed).
--
-- One row per firing (tick, run-now, or resumed). NOT dry-run/test-fire —
-- those are side-effect-free and record nothing. Mirrors mcp_tool_calls /
-- workflow_runs (owner-scoped history + terminal status). Surfaced per-task in
-- the task drawer's "Runs" tab; refreshed live and time-pruned alongside
-- notifications.

CREATE TABLE scheduled_task_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    scheduled_task_id UUID NOT NULL REFERENCES scheduled_tasks(id) ON DELETE CASCADE,
    -- Denormalized owner so the audit list is a single-table owner-scoped query
    -- and survives via the task FK cascade semantics.
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- How this firing was triggered.
    trigger TEXT NOT NULL DEFAULT 'schedule'
        CHECK (trigger IN ('schedule', 'run_now', 'catchup')),

    status TEXT NOT NULL
        CHECK (status IN ('completed', 'no_change', 'failed')),
    -- Failure taxonomy bucket (DEC-18): 'transient' | 'auth' | 'permission' |
    -- 'validation' | 'target_missing' | 'internal'. NULL on success.
    error_class TEXT,
    error_message TEXT,

    -- Links to the produced output (SET NULL so GC of the linked entity doesn't
    -- orphan the audit row).
    notification_id UUID REFERENCES notifications(id) ON DELETE SET NULL,
    workflow_run_id UUID REFERENCES workflow_runs(id) ON DELETE SET NULL,
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,

    fired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);

COMMENT ON TABLE scheduled_task_runs IS
    'Per-firing audit history for a scheduled task (Runs tab); excludes side-effect-free dry-runs.';

-- Per-task history, newest first.
CREATE INDEX idx_scheduled_task_runs_task ON scheduled_task_runs(scheduled_task_id, fired_at DESC);
-- Owner-scoped + retention prune scan.
CREATE INDEX idx_scheduled_task_runs_user_fired ON scheduled_task_runs(user_id, fired_at DESC);
