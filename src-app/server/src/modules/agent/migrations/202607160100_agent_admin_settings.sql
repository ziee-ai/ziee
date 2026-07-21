-- agent module: deployment-wide agent policy singleton (DEC-6 / ITEM-28).
--
-- One row keyed on a boolean PK pinned to TRUE (the `id = true` idiom used by
-- `code_sandbox_settings` / `js_tool_settings`) so there is exactly one config
-- row. Admin-only surface (`agent::settings::{read,manage}`), no per-user rows.
-- Bounds are enforced both here (DB CHECK backstop) and in the PUT handler's
-- `validate()` (clean 400 before any write).

CREATE TABLE public.agent_admin_settings (
    id boolean DEFAULT true NOT NULL,
    -- Codex SandboxMode: how much filesystem/network an unattended run may touch.
    default_sandbox_mode text DEFAULT 'workspace-write'::text NOT NULL,
    -- Policy for a mutating/external tool call in an unattended run (DEC-1).
    unattended_approval_policy text DEFAULT 'on-request'::text NOT NULL,
    -- Reviewer agent (DEC-3): on by default, fail-closed.
    reviewer_enabled boolean DEFAULT true NOT NULL,
    -- NULL ⇒ fall back to the run's own model.
    reviewer_model_id uuid,
    -- Free-text steering for the reviewer's risk classification (NULL ⇒ default).
    reviewer_policy text,
    -- Per-risk-band → decision overrides (e.g. {"high":"prompt"}).
    reviewer_risk_thresholds jsonb DEFAULT '{}'::jsonb NOT NULL,
    -- Token ceilings (the real cost bound; Goose's step-cap is the failsafe).
    per_run_token_cap bigint DEFAULT 5000000 NOT NULL,
    per_step_token_cap bigint DEFAULT 2000000 NOT NULL,
    -- Iteration failsafe (DEC-7).
    default_max_steps integer DEFAULT 30 NOT NULL,
    -- Fan-out guardrails (DEC-11).
    fan_out_max_threads integer DEFAULT 6 NOT NULL,
    fan_out_max_depth integer DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT agent_admin_settings_id_check CHECK ((id = true)),
    CONSTRAINT agent_admin_settings_default_sandbox_mode_check
        CHECK ((default_sandbox_mode = ANY (ARRAY['read-only'::text, 'workspace-write'::text, 'danger-full-access'::text]))),
    CONSTRAINT agent_admin_settings_unattended_approval_policy_check
        CHECK ((unattended_approval_policy = ANY (ARRAY['untrusted'::text, 'on-failure'::text, 'on-request'::text, 'never'::text]))),
    CONSTRAINT agent_admin_settings_reviewer_policy_len CHECK ((reviewer_policy IS NULL OR length(reviewer_policy) <= 32768)),
    CONSTRAINT agent_admin_settings_per_run_token_cap_check CHECK (((per_run_token_cap >= 1000) AND (per_run_token_cap <= 1000000000))),
    CONSTRAINT agent_admin_settings_per_step_token_cap_check CHECK (((per_step_token_cap >= 1000) AND (per_step_token_cap <= 1000000000))),
    CONSTRAINT agent_admin_settings_default_max_steps_check CHECK (((default_max_steps >= 1) AND (default_max_steps <= 1000))),
    CONSTRAINT agent_admin_settings_fan_out_max_threads_check CHECK (((fan_out_max_threads >= 1) AND (fan_out_max_threads <= 64))),
    CONSTRAINT agent_admin_settings_fan_out_max_depth_check CHECK (((fan_out_max_depth >= 1) AND (fan_out_max_depth <= 5)))
);

ALTER TABLE ONLY public.agent_admin_settings
    ADD CONSTRAINT agent_admin_settings_pkey PRIMARY KEY (id);

INSERT INTO agent_admin_settings (id) VALUES (true) ON CONFLICT (id) DO NOTHING;
