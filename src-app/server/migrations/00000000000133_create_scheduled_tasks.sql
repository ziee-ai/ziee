-- Scheduled / recurring background tasks (scheduler module).
--
-- A user-owned "task" that fires on a schedule (once at a time, or recurring
-- via cron) and runs one of the existing execution seams with NO browser
-- attached:
--   * target_kind='workflow' — runs a saved workflow (workflow_id + inputs +
--     model) via workflow::runner::spawn_run (invocation_source='scheduled').
--   * target_kind='prompt'   — runs one assistant turn (assistant + prompt +
--     model) via the chat pipeline, appended to a per-task BOUND conversation
--     so follow-up is native.
--
-- The boot-spawned tick loop (modules/scheduler/tick.rs) claims due rows with
-- FOR UPDATE SKIP LOCKED (mirrors memory/reaper.rs), dispatches, and advances
-- next_run_at. Downtime is handled by coalesced catch-up: an overdue task
-- fires ONCE then advances past now (never backfills N occurrences).
--
-- Owner-scoped: every query is WHERE user_id = $1. Realtime via
-- SyncEntity::ScheduledTask (Audience::owner).
--
-- NOTE: all feature columns live here in the create (not a follow-on ALTER) —
-- these migrations ship together, so a fresh table with its full shape is
-- clearer than ALTERing a same-batch table. The per-firing audit table
-- (scheduled_task_runs) is migration 137.

CREATE TABLE scheduled_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Owner. The task's target executes AS this user (workflow execute /
    -- model access is re-checked downstream at spawn time).
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    name VARCHAR(255) NOT NULL,
    -- enabled=false is a user pause (disabled tasks never fire). paused_reason
    -- records an AUTO pause (max failures / bound conversation deleted); a
    -- non-NULL paused_reason implies enabled=false.
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    paused_reason TEXT,

    -- ── Target ──────────────────────────────────────────────────────────
    target_kind TEXT NOT NULL CHECK (target_kind IN ('workflow', 'prompt')),
    -- workflow target: SET NULL so deleting the workflow/model auto-pauses the
    -- task (the tick treats a NULL target as a terminal error) rather than
    -- cascade-deleting the task.
    workflow_id UUID REFERENCES workflows(id) ON DELETE SET NULL,
    inputs_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- prompt target: assistant is optional (NULL => the user's default
    -- assistant is resolved at fire time); prompt is the user message text.
    assistant_id UUID REFERENCES assistants(id) ON DELETE SET NULL,
    prompt TEXT,
    -- Shared: a scheduled run has no conversation to inherit a model from, so
    -- model_id is required at create time (validated against the user's access)
    -- and snapshot here. SET NULL => auto-pause on model delete.
    model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL,

    -- ── Schedule ────────────────────────────────────────────────────────
    schedule_kind TEXT NOT NULL CHECK (schedule_kind IN ('once', 'recurring')),
    -- once: a single UTC instant.
    run_at TIMESTAMPTZ,
    -- recurring: a 5-field POSIX/Vixie cron (croner) + IANA timezone.
    cron_expr TEXT,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    -- Computed next fire instant (always UTC). NULL once a 'once' task has
    -- fired or a task is terminally disabled. Indexed for the due-scan.
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    last_status TEXT,

    -- ── Failure handling (DEC-18) ───────────────────────────────────────
    consecutive_failures INTEGER NOT NULL DEFAULT 0,

    -- ── Delivery + change-detection (DEC-19 / DEC-20) ───────────────────
    -- notify_mode: always => durable inbox row + live toast/badge; silent =>
    -- durable row only (auditable, non-interrupting).
    notify_mode TEXT NOT NULL DEFAULT 'always'
        CHECK (notify_mode IN ('always', 'silent')),
    -- notify_on: always => notify every success; on_change => only when the
    -- result differs from last run (fingerprint / item-set diff).
    notify_on TEXT NOT NULL DEFAULT 'always'
        CHECK (notify_on IN ('always', 'on_change')),
    last_result_fingerprint TEXT,
    last_result_signature_json JSONB,

    -- prompt-kind bound conversation (DEC-17): the one conversation each run
    -- appends to. Deleting it auto-pauses the task (detected at tick time; SET
    -- NULL keeps the row). NULL for workflow-kind and before the first run.
    bound_conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Target-kind ↔ required-field coherence (defense-in-depth; the handler
    -- validates first for clearer errors).
    CONSTRAINT scheduled_tasks_target_coherent CHECK (
        (target_kind = 'workflow' AND workflow_id IS NOT NULL)
        OR (target_kind = 'prompt' AND prompt IS NOT NULL)
    ),
    CONSTRAINT scheduled_tasks_schedule_coherent CHECK (
        (schedule_kind = 'once' AND run_at IS NOT NULL)
        OR (schedule_kind = 'recurring' AND cron_expr IS NOT NULL)
    )
);

COMMENT ON TABLE scheduled_tasks IS
    'User-owned scheduled/recurring background tasks (workflow or prompt target); fired by the scheduler tick loop.';

-- The due-scan: enabled tasks whose next fire instant has arrived, soonest
-- first. Partial index keeps it tight (disabled/terminal tasks excluded).
CREATE INDEX idx_scheduled_tasks_due
    ON scheduled_tasks(next_run_at)
    WHERE enabled AND next_run_at IS NOT NULL;

-- Owner list query.
CREATE INDEX idx_scheduled_tasks_user ON scheduled_tasks(user_id, created_at DESC);
