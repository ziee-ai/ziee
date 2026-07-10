-- Scheduler admin settings (singleton deployment-wide config).
--
-- Mirrors memory_admin_settings / session_settings: a single row (id=TRUE)
-- read fresh each create/tick so admin changes take effect without a restart.
-- Governs the per-user active-task quota, the abusive-cadence floor, the
-- consecutive-failure auto-pause threshold, and notification retention.

CREATE TABLE scheduler_admin_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- Max ENABLED tasks a single user may hold; create returns 422 past this.
    max_active_tasks_per_user   INTEGER NOT NULL DEFAULT 20,
    -- Reject recurring cadences tighter than this (abuse floor).
    min_interval_seconds        INTEGER NOT NULL DEFAULT 300,
    -- Auto-pause a task after this many consecutive failed firings (flap cap).
    max_consecutive_failures    INTEGER NOT NULL DEFAULT 5,
    -- Notification retention; 0 = keep forever.
    notification_retention_days INTEGER NOT NULL DEFAULT 30,

    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT scheduler_max_active_range
        CHECK (max_active_tasks_per_user BETWEEN 1 AND 1000),
    CONSTRAINT scheduler_min_interval_range
        CHECK (min_interval_seconds BETWEEN 60 AND 86400),
    CONSTRAINT scheduler_max_failures_range
        CHECK (max_consecutive_failures BETWEEN 1 AND 100),
    CONSTRAINT scheduler_retention_range
        CHECK (notification_retention_days BETWEEN 0 AND 3650)
);

COMMENT ON TABLE scheduler_admin_settings IS
    'Singleton deployment-wide scheduler config (quota, cadence floor, failure cap, notification retention).';

INSERT INTO scheduler_admin_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;
