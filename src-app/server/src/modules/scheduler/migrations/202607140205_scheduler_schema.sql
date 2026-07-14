-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- scheduler module tables + indexes + triggers.

CREATE TABLE public.scheduled_task_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    scheduled_task_id uuid NOT NULL,
    user_id uuid NOT NULL,
    trigger text DEFAULT 'schedule'::text NOT NULL,
    status text NOT NULL,
    error_class text,
    error_message text,
    notification_id uuid,
    workflow_run_id uuid,
    conversation_id uuid,
    fired_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    skipped_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    result_preview text,
    change_summary_json jsonb,
    CONSTRAINT scheduled_task_runs_status_check CHECK ((status = ANY (ARRAY['completed'::text, 'no_change'::text, 'failed'::text]))),
    CONSTRAINT scheduled_task_runs_trigger_check CHECK ((trigger = ANY (ARRAY['schedule'::text, 'run_now'::text, 'catchup'::text])))
);

CREATE TABLE public.scheduled_tasks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    paused_reason text,
    target_kind text NOT NULL,
    workflow_id uuid,
    inputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    assistant_id uuid,
    prompt text,
    model_id uuid,
    schedule_kind text NOT NULL,
    run_at timestamp with time zone,
    cron_expr text,
    timezone text DEFAULT 'UTC'::text NOT NULL,
    next_run_at timestamp with time zone,
    last_run_at timestamp with time zone,
    last_status text,
    consecutive_failures integer DEFAULT 0 NOT NULL,
    notify_mode text DEFAULT 'always'::text NOT NULL,
    notify_on text DEFAULT 'always'::text NOT NULL,
    last_result_fingerprint text,
    last_result_signature_json jsonb,
    bound_conversation_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    allowed_unattended_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    CONSTRAINT scheduled_tasks_notify_mode_check CHECK ((notify_mode = ANY (ARRAY['always'::text, 'silent'::text]))),
    CONSTRAINT scheduled_tasks_notify_on_check CHECK ((notify_on = ANY (ARRAY['always'::text, 'on_change'::text]))),
    CONSTRAINT scheduled_tasks_schedule_coherent CHECK ((((schedule_kind = 'once'::text) AND (run_at IS NOT NULL)) OR ((schedule_kind = 'recurring'::text) AND (cron_expr IS NOT NULL)))),
    CONSTRAINT scheduled_tasks_schedule_kind_check CHECK ((schedule_kind = ANY (ARRAY['once'::text, 'recurring'::text]))),
    CONSTRAINT scheduled_tasks_target_coherent CHECK ((((target_kind = 'workflow'::text) AND (workflow_id IS NOT NULL)) OR ((target_kind = 'prompt'::text) AND (prompt IS NOT NULL)))),
    CONSTRAINT scheduled_tasks_target_kind_check CHECK ((target_kind = ANY (ARRAY['workflow'::text, 'prompt'::text])))
);

CREATE TABLE public.scheduler_admin_settings (
    id boolean DEFAULT true NOT NULL,
    max_active_tasks_per_user integer DEFAULT 20 NOT NULL,
    min_interval_seconds integer DEFAULT 300 NOT NULL,
    max_consecutive_failures integer DEFAULT 5 NOT NULL,
    notification_retention_days integer DEFAULT 30 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT scheduler_admin_settings_id_check CHECK ((id = true)),
    CONSTRAINT scheduler_max_active_range CHECK (((max_active_tasks_per_user >= 1) AND (max_active_tasks_per_user <= 1000))),
    CONSTRAINT scheduler_max_failures_range CHECK (((max_consecutive_failures >= 1) AND (max_consecutive_failures <= 100))),
    CONSTRAINT scheduler_min_interval_range CHECK (((min_interval_seconds >= 60) AND (min_interval_seconds <= 86400))),
    CONSTRAINT scheduler_retention_range CHECK (((notification_retention_days >= 0) AND (notification_retention_days <= 3650)))
);

ALTER TABLE ONLY public.scheduled_task_runs
    ADD CONSTRAINT scheduled_task_runs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.scheduler_admin_settings
    ADD CONSTRAINT scheduler_admin_settings_pkey PRIMARY KEY (id);

CREATE INDEX idx_scheduled_task_runs_task ON public.scheduled_task_runs USING btree (scheduled_task_id, fired_at DESC);

CREATE INDEX idx_scheduled_task_runs_user_fired ON public.scheduled_task_runs USING btree (user_id, fired_at DESC);

CREATE INDEX idx_scheduled_tasks_due ON public.scheduled_tasks USING btree (next_run_at) WHERE (enabled AND (next_run_at IS NOT NULL));

CREATE INDEX idx_scheduled_tasks_user ON public.scheduled_tasks USING btree (user_id, created_at DESC);
