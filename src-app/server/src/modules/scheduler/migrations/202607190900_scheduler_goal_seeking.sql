-- scheduler: goal-seeking completion condition (ITEM-24 / DEC-61/62/63).
--
-- A GOAL-SEEKING task is a `self_paced` prompt task that additionally carries a
-- natural-language `completion_condition` (the "done when…" rubric). After each
-- fired turn an isolated, cheap, INDEPENDENT model evaluator judges the turn's
-- result against this condition:
--   * done     → self-stop the loop (reuse the self-paced Disable path →
--                paused_reason='completed').
--   * not_done → re-arm another turn (reuse the self-paced write-back clamp)
--                UNTIL `agent_admin_settings.goal_seek_max_turns` OR the
--                `max_horizon_days` backstop is hit → stop 'incomplete'.
-- On evaluator error/timeout the verdict is not_done (keep working; never a
-- false "done"). NULLABLE — a task WITHOUT a condition is an ordinary
-- once/recurring/self_paced task, unchanged.
ALTER TABLE public.scheduled_tasks
    ADD COLUMN completion_condition text;

-- Defense-in-depth length backstop (the handler validates first → clean 400).
ALTER TABLE public.scheduled_tasks
    ADD CONSTRAINT scheduled_tasks_completion_condition_len
        CHECK ((completion_condition IS NULL OR length(completion_condition) <= 4096));
