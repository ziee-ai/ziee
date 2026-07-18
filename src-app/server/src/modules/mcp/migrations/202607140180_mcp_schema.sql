-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- mcp module tables + indexes + triggers.

CREATE TABLE public.mcp_server_oauth_configs (
    server_id uuid NOT NULL,
    client_id text NOT NULL,
    client_secret text,
    scopes text,
    resource text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    client_secret_encrypted bytea
);

CREATE TABLE public.mcp_servers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    name character varying(255) NOT NULL,
    display_name character varying(255) NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    is_system boolean DEFAULT false NOT NULL,
    transport_type character varying(50) DEFAULT 'stdio'::character varying NOT NULL,
    command text,
    args jsonb DEFAULT '[]'::jsonb,
    environment_variables jsonb DEFAULT '{}'::jsonb,
    url text,
    headers jsonb DEFAULT '{}'::jsonb,
    timeout_seconds integer DEFAULT 30 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    supports_sampling boolean DEFAULT false NOT NULL,
    usage_mode character varying(50) DEFAULT 'auto'::character varying NOT NULL,
    max_concurrent_sessions integer,
    is_built_in boolean DEFAULT false NOT NULL,
    run_in_sandbox boolean DEFAULT false NOT NULL,
    environment_variables_encrypted jsonb DEFAULT '{}'::jsonb NOT NULL,
    environment_variables_secret_keys text[] DEFAULT '{}'::text[] NOT NULL,
    headers_encrypted jsonb DEFAULT '{}'::jsonb NOT NULL,
    headers_secret_keys text[] DEFAULT '{}'::text[] NOT NULL,
    last_health_check_at timestamp with time zone,
    last_health_check_status text DEFAULT 'untested'::text NOT NULL,
    last_health_check_reason text,
    sandbox_flavor character varying(32) DEFAULT 'full'::character varying NOT NULL,
    CONSTRAINT mcp_servers_last_health_check_status_check CHECK ((last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text]))),
    CONSTRAINT mcp_servers_usage_mode_check CHECK ((usage_mode IN ('auto', 'always'))),
    CONSTRAINT system_server_must_have_no_owner CHECK ((((is_system = true) AND (user_id IS NULL)) OR ((is_system = false) AND (user_id IS NOT NULL)))),
    CONSTRAINT valid_transport_config CHECK (((((transport_type)::text = 'stdio'::text) AND (command IS NOT NULL)) OR ((transport_type IN ('http', 'sse')) AND (url IS NOT NULL))))
);

CREATE TABLE public.mcp_settings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid,
    project_id uuid,
    user_id uuid NOT NULL,
    approval_mode character varying(50) DEFAULT 'manual_approve'::character varying NOT NULL,
    auto_approved_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    disabled_servers jsonb DEFAULT '[]'::jsonb NOT NULL,
    loop_settings jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT mcp_settings_one_scope CHECK (((conversation_id IS NULL) <> (project_id IS NULL)))
);

CREATE TABLE public.mcp_tool_calls (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    server_id uuid,
    server_name character varying(255) NOT NULL,
    is_built_in boolean DEFAULT false NOT NULL,
    user_id uuid NOT NULL,
    conversation_id uuid,
    branch_id uuid,
    message_id uuid,
    tool_use_id character varying(255),
    tool_name character varying(255) NOT NULL,
    arguments_json jsonb DEFAULT '{}'::jsonb NOT NULL,
    source character varying(20) DEFAULT 'chat'::character varying NOT NULL,
    status character varying(20) DEFAULT 'completed'::character varying NOT NULL,
    is_error boolean DEFAULT false NOT NULL,
    result_json jsonb,
    content_kinds text[] DEFAULT '{}'::text[] NOT NULL,
    result_bytes bigint DEFAULT 0 NOT NULL,
    error_message text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    duration_ms bigint,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    workflow_run_id uuid,
    CONSTRAINT mcp_tool_calls_source_check CHECK ((source IN ('chat', 'rest', 'always', 'sampling', 'approval', 'workflow', 'script'))),
    CONSTRAINT mcp_tool_calls_status_check CHECK ((status IN ('completed', 'failed', 'timeout', 'cancelled')))
);

CREATE TABLE public.mcp_user_policy (
    id integer DEFAULT 1 NOT NULL,
    allowed_transports text[] DEFAULT ARRAY['http'::text, 'stdio'::text] NOT NULL,
    user_stdio_sandbox_flavor text DEFAULT 'full'::text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_by uuid,
    tool_call_retention_days integer DEFAULT 90 NOT NULL,
    CONSTRAINT mcp_user_policy_id_check CHECK ((id = 1))
);

CREATE TABLE public.tool_use_approvals (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid NOT NULL,
    branch_id uuid NOT NULL,
    message_id uuid NOT NULL,
    user_id uuid NOT NULL,
    tool_use_id character varying(255) NOT NULL,
    tool_name character varying(255) NOT NULL,
    tool_input jsonb NOT NULL,
    server_id uuid,
    server_name character varying(255) NOT NULL,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    approved_at timestamp with time zone,
    approved_by uuid,
    approval_note text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.user_group_mcp_servers (
    group_id uuid NOT NULL,
    mcp_server_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.user_mcp_defaults (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    approval_mode character varying(50) DEFAULT 'manual_approve'::character varying NOT NULL,
    auto_approved_tools jsonb DEFAULT '[]'::jsonb NOT NULL,
    disabled_servers jsonb DEFAULT '[]'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    loop_settings jsonb
);

ALTER TABLE ONLY public.mcp_server_oauth_configs
    ADD CONSTRAINT mcp_server_oauth_configs_pkey PRIMARY KEY (server_id);

ALTER TABLE ONLY public.mcp_servers
    ADD CONSTRAINT mcp_servers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_one_per_conversation UNIQUE (conversation_id);

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_one_per_project UNIQUE (project_id);

ALTER TABLE ONLY public.mcp_settings
    ADD CONSTRAINT mcp_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.mcp_tool_calls
    ADD CONSTRAINT mcp_tool_calls_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.mcp_user_policy
    ADD CONSTRAINT mcp_user_policy_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT tool_use_approvals_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.tool_use_approvals
    ADD CONSTRAINT unique_tool_use UNIQUE (message_id, tool_use_id);

ALTER TABLE ONLY public.user_mcp_defaults
    ADD CONSTRAINT unique_user_mcp_defaults UNIQUE (user_id);

ALTER TABLE ONLY public.user_group_mcp_servers
    ADD CONSTRAINT user_group_mcp_servers_pkey PRIMARY KEY (group_id, mcp_server_id);

ALTER TABLE ONLY public.user_mcp_defaults
    ADD CONSTRAINT user_mcp_defaults_pkey PRIMARY KEY (id);

CREATE INDEX idx_group_mcp_servers_group_id ON public.user_group_mcp_servers USING btree (group_id);

CREATE INDEX idx_group_mcp_servers_server_id ON public.user_group_mcp_servers USING btree (mcp_server_id);

CREATE INDEX idx_mcp_servers_enabled ON public.mcp_servers USING btree (enabled);

CREATE INDEX idx_mcp_servers_health_status_unhealthy ON public.mcp_servers USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text);

CREATE INDEX idx_mcp_servers_is_built_in ON public.mcp_servers USING btree (is_built_in);

CREATE INDEX idx_mcp_servers_is_system ON public.mcp_servers USING btree (is_system);

CREATE INDEX idx_mcp_servers_transport_type ON public.mcp_servers USING btree (transport_type);

CREATE INDEX idx_mcp_servers_user_id ON public.mcp_servers USING btree (user_id);

CREATE INDEX idx_mcp_settings_user_id ON public.mcp_settings USING btree (user_id);

CREATE INDEX idx_mcp_tool_calls_conv ON public.mcp_tool_calls USING btree (conversation_id) WHERE (conversation_id IS NOT NULL);

CREATE INDEX idx_mcp_tool_calls_created ON public.mcp_tool_calls USING btree (created_at);

CREATE INDEX idx_mcp_tool_calls_server ON public.mcp_tool_calls USING btree (server_id);

CREATE INDEX idx_mcp_tool_calls_user_created ON public.mcp_tool_calls USING btree (user_id, created_at DESC);

CREATE INDEX idx_mcp_tool_calls_workflow_run ON public.mcp_tool_calls USING btree (workflow_run_id) WHERE (workflow_run_id IS NOT NULL);

CREATE INDEX idx_tool_use_approvals_branch_id ON public.tool_use_approvals USING btree (branch_id);

CREATE INDEX idx_tool_use_approvals_branch_status ON public.tool_use_approvals USING btree (branch_id, status);

CREATE INDEX idx_tool_use_approvals_conversation_id ON public.tool_use_approvals USING btree (conversation_id);

CREATE INDEX idx_tool_use_approvals_message_id ON public.tool_use_approvals USING btree (message_id);

CREATE INDEX idx_tool_use_approvals_server_id ON public.tool_use_approvals USING btree (server_id);

CREATE INDEX idx_tool_use_approvals_status ON public.tool_use_approvals USING btree (status);

CREATE INDEX idx_tool_use_approvals_user_id ON public.tool_use_approvals USING btree (user_id);

CREATE TRIGGER update_mcp_settings_updated_at BEFORE UPDATE ON public.mcp_settings FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();

CREATE TRIGGER update_mcp_tool_calls_updated_at BEFORE UPDATE ON public.mcp_tool_calls FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();

CREATE TRIGGER update_tool_use_approvals_updated_at BEFORE UPDATE ON public.tool_use_approvals FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();

CREATE TRIGGER update_user_mcp_defaults_updated_at BEFORE UPDATE ON public.user_mcp_defaults FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
