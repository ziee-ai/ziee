-- agent module: goal-seeking evaluator settings (ITEM-24 / DEC-61/62).
--
-- `goal_eval_model_id` (NULL ⇒ the goal-seeking run's OWN model — mirrors
-- `reviewer_model_id`) is the cheap, independent model that judges a fired
-- turn's result against the task's `completion_condition`. Resolved under the
-- run owner's RBAC at evaluation time.
--
-- `goal_seek_max_turns` bounds how many turns a goal-seeking loop may fire
-- before stopping 'incomplete' (the `scheduler_admin_settings.max_horizon_days`
-- 7-day backstop is the other ceiling). Default 10, range 1..=50 — mirrors the
-- iteration failsafe pattern of `default_max_steps`.
ALTER TABLE public.agent_admin_settings
    ADD COLUMN goal_eval_model_id uuid;

ALTER TABLE public.agent_admin_settings
    ADD COLUMN goal_seek_max_turns integer DEFAULT 10 NOT NULL;

ALTER TABLE public.agent_admin_settings
    ADD CONSTRAINT agent_admin_settings_goal_seek_max_turns_check
        CHECK (((goal_seek_max_turns >= 1) AND (goal_seek_max_turns <= 50)));
