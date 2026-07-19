-- agent module: per-`delegate`-call child-count cap (DEC-1 / ITEM-3 admin side).
--
-- `fan_out_max_threads` bounds fan-out CONCURRENCY; this bounds the COUNT of
-- children accepted in ONE `delegate` call (over-cap truncates with an explicit
-- "capped at N" note — never a silent drop). Threaded from here into the crate's
-- `SubagentLimits.max_children_per_call` by `workflow/agent_dispatch.rs`.
-- Bounds mirror the PUT handler's `validate()` (1..=64).

ALTER TABLE public.agent_admin_settings
    ADD COLUMN fan_out_max_children_per_call integer DEFAULT 8 NOT NULL;

ALTER TABLE public.agent_admin_settings
    ADD CONSTRAINT agent_admin_settings_fan_out_max_children_per_call_check
        CHECK (((fan_out_max_children_per_call >= 1) AND (fan_out_max_children_per_call <= 64)));
