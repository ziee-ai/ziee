-- Notification inbox (notification module).
--
-- A durable, owner-scoped inbox where background results land ("your literature
-- sweep found 12 new papers"). Greenfield — there was no notification/inbox
-- concept before. Structurally mirrors mcp_tool_calls (owner-scoped history +
-- retention prune). New rows are pushed live via SyncEntity::Notification
-- (Audience::owner, origin=None); a missed toast is still readable here later.
--
-- The scheduler is the first producer, but the module is general: other
-- background completions (a long download, a manual workflow run) can write
-- here later. Retention: scheduler_admin_settings.notification_retention_days
-- (0 = keep forever), pruned by modules/notification/prune.rs.

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- Discriminator for icon/grouping, e.g. 'scheduled_task_result',
    -- 'scheduled_task_failed', 'scheduled_task_no_change'.
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',

    -- Deep-link targets (the notification is a POINTER to the real output the
    -- user verifies; SET NULL so deleting the linked entity doesn't orphan the
    -- notification — it just loses its link).
    scheduled_task_id UUID REFERENCES scheduled_tasks(id) ON DELETE SET NULL,
    workflow_run_id UUID REFERENCES workflow_runs(id) ON DELETE SET NULL,
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,

    -- NULL => unread.
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE notifications IS
    'Durable owner-scoped notification inbox for background results (scheduler + future producers).';

-- Primary list query: a user's inbox, newest first.
CREATE INDEX idx_notifications_user_created ON notifications(user_id, created_at DESC);

-- Unread-count query (badge).
CREATE INDEX idx_notifications_user_unread
    ON notifications(user_id)
    WHERE read_at IS NULL;
