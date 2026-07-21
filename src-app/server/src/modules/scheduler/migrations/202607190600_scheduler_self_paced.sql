-- scheduler: self-paced schedule kind (ITEM-21 / DEC-42/44/45/48) + the
-- max-horizon self-stop backstop + the bound-conversation filter index
-- (ITEM-23 / DEC-47).
--
-- A 'self_paced' task carries NEITHER run_at NOR cron_expr at rest: after each
-- fired turn the model proposes the next delay (clamped to
-- [min_interval_seconds, max_horizon_days]) or stops, and the write-back path
-- re-arms next_run_at. So the schedule-coherence CHECK is relaxed for it.
-- Uses the DROP + re-ADD CONSTRAINT pattern from 202607170100.

-- Admit 'self_paced' as a schedule kind.
ALTER TABLE public.scheduled_tasks
    DROP CONSTRAINT scheduled_tasks_schedule_kind_check;
ALTER TABLE public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_schedule_kind_check
        CHECK ((schedule_kind = ANY (ARRAY['once'::text, 'recurring'::text, 'self_paced'::text])));

-- Relax schedule coherence: 'once' still needs run_at, 'recurring' still needs
-- cron_expr, but 'self_paced' needs neither (its cadence is model-driven).
ALTER TABLE public.scheduled_tasks
    DROP CONSTRAINT scheduled_tasks_schedule_coherent;
ALTER TABLE public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_schedule_coherent
        CHECK (
            (((schedule_kind = 'once'::text) AND (run_at IS NOT NULL))
             OR ((schedule_kind = 'recurring'::text) AND (cron_expr IS NOT NULL))
             OR (schedule_kind = 'self_paced'::text))
        );

-- Absolute self-paced backstop (DEC-45): the model's proposed delay is clamped
-- to at most this many days, and a self-paced task self-stops max_horizon_days
-- after creation. Default 7, range 1..=365.
ALTER TABLE public.scheduler_admin_settings
    ADD COLUMN max_horizon_days integer DEFAULT 7 NOT NULL;
ALTER TABLE public.scheduler_admin_settings
    ADD CONSTRAINT scheduler_max_horizon_range
        CHECK (((max_horizon_days >= 1) AND (max_horizon_days <= 365)));

-- ITEM-23 / DEC-47: filter a user's tasks by their bound conversation (the
-- in-chat "attached loops/schedules" list). Partial (bound rows only).
CREATE INDEX idx_scheduled_tasks_bound_conversation
    ON public.scheduled_tasks USING btree (bound_conversation_id)
    WHERE (bound_conversation_id IS NOT NULL);
