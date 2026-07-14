-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- workflow module tables + indexes + triggers.

CREATE FUNCTION public.enforce_system_scope_for_group_workflows() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (SELECT scope FROM workflows WHERE id = NEW.workflow_id) <> 'system' THEN
        RAISE EXCEPTION 'group_workflows: only system-scope workflows can be assigned to groups (workflow_id=%)', NEW.workflow_id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TABLE public.group_workflows (
    group_id uuid NOT NULL,
    workflow_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.workflow_runs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    workflow_id uuid NOT NULL,
    conversation_id uuid,
    user_id uuid NOT NULL,
    model_id uuid,
    sandbox_flavor text,
    run_kind character varying(10) DEFAULT 'normal'::character varying NOT NULL,
    inputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_outputs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_item_progress_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_logs_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    step_artifacts_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    pending_elicitation_json jsonb,
    final_output_json jsonb,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    current_step text,
    error_message text,
    total_tokens bigint DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    invocation_source character varying(20) DEFAULT 'manual'::character varying NOT NULL,
    step_progress_json jsonb,
    elicit_response_json jsonb,
    CONSTRAINT workflow_runs_invocation_source_check CHECK ((invocation_source IN ('manual', 'conversation', 'agent', 'mcp_tool', 'scheduled'))),
    CONSTRAINT workflow_runs_run_kind_check CHECK ((run_kind IN ('normal', 'test', 'dry_run'))),
    CONSTRAINT workflow_runs_status_check CHECK ((status IN ('pending', 'running', 'waiting', 'completed', 'failed', 'cancelled')))
);

CREATE TABLE public.workflows (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    version text,
    display_name text,
    description text,
    extracted_path text NOT NULL,
    bundle_sha256 text NOT NULL,
    bundle_size_bytes bigint NOT NULL,
    file_count integer NOT NULL,
    entry_point text NOT NULL,
    tags jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope character varying(10) DEFAULT 'user'::character varying NOT NULL,
    owner_user_id uuid,
    created_by uuid,
    enabled boolean DEFAULT true NOT NULL,
    is_dev boolean DEFAULT false NOT NULL,
    compiled_ir_json jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    ephemeral boolean DEFAULT false NOT NULL,
    conversation_id uuid,
    CONSTRAINT workflows_scope_check CHECK ((scope IN ('user', 'system'))),
    CONSTRAINT workflows_scope_owner_check CHECK (((((scope)::text = 'user'::text) AND (owner_user_id IS NOT NULL)) OR (((scope)::text = 'system'::text) AND (owner_user_id IS NULL))))
);

ALTER TABLE ONLY public.group_workflows
    ADD CONSTRAINT group_workflows_pkey PRIMARY KEY (group_id, workflow_id);

ALTER TABLE ONLY public.workflow_runs
    ADD CONSTRAINT workflow_runs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.workflows
    ADD CONSTRAINT workflows_pkey PRIMARY KEY (id);

CREATE INDEX idx_group_workflows_workflow ON public.group_workflows USING btree (workflow_id);

CREATE INDEX idx_workflow_runs_conv ON public.workflow_runs USING btree (conversation_id) WHERE (conversation_id IS NOT NULL);

CREATE INDEX idx_workflow_runs_created_at ON public.workflow_runs USING btree (created_at);

CREATE INDEX idx_workflow_runs_history ON public.workflow_runs USING btree (workflow_id, user_id, created_at DESC);

CREATE INDEX idx_workflow_runs_run_kind ON public.workflow_runs USING btree (run_kind);

CREATE INDEX idx_workflow_runs_status ON public.workflow_runs USING btree (status);

CREATE INDEX idx_workflow_runs_user ON public.workflow_runs USING btree (user_id);

CREATE INDEX idx_workflow_runs_user_created ON public.workflow_runs USING btree (user_id, created_at DESC);

CREATE INDEX idx_workflow_runs_workflow ON public.workflow_runs USING btree (workflow_id);

CREATE INDEX idx_workflows_conversation ON public.workflows USING btree (conversation_id) WHERE (ephemeral = true);

CREATE INDEX idx_workflows_name ON public.workflows USING btree (name);

CREATE INDEX idx_workflows_owner ON public.workflows USING btree (owner_user_id) WHERE ((scope)::text = 'user'::text);

CREATE UNIQUE INDEX uniq_workflows_system_name_version ON public.workflows USING btree (name, version) WHERE ((scope)::text = 'system'::text);

CREATE UNIQUE INDEX uniq_workflows_user_name_version_owner ON public.workflows USING btree (name, version, owner_user_id) WHERE ((scope)::text = 'user'::text);

CREATE TRIGGER group_workflows_scope_check BEFORE INSERT OR UPDATE ON public.group_workflows FOR EACH ROW EXECUTE FUNCTION public.enforce_system_scope_for_group_workflows();
