-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_local_runtime module tables + indexes + triggers.

CREATE TABLE public.llm_runtime_instances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    local_port integer NOT NULL,
    base_url character varying(512) NOT NULL,
    status character varying(50) NOT NULL,
    error_message text,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    last_health_check timestamp with time zone,
    stopped_at timestamp with time zone,
    runtime_version_id uuid,
    state character varying(50) DEFAULT 'starting'::character varying NOT NULL,
    state_changed_at timestamp with time zone DEFAULT now() NOT NULL,
    restart_attempts integer DEFAULT 0 NOT NULL,
    last_failure_reason text,
    last_used_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_instances_state_check CHECK ((state IN ('starting', 'healthy', 'unhealthy', 'crashed', 'restarting', 'failed', 'stopped'))),
    CONSTRAINT llm_runtime_instances_status_check CHECK ((status IN ('starting', 'running', 'stopping', 'stopped', 'failed')))
);

CREATE TABLE public.llm_runtime_settings (
    id boolean DEFAULT true NOT NULL,
    idle_unload_secs integer DEFAULT 1800 NOT NULL,
    auto_start_timeout_secs integer DEFAULT 30 NOT NULL,
    drain_timeout_secs integer DEFAULT 30 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_settings_auto_start_timeout_secs_check CHECK (((auto_start_timeout_secs >= 1) AND (auto_start_timeout_secs <= 600))),
    CONSTRAINT llm_runtime_settings_drain_timeout_secs_check CHECK (((drain_timeout_secs >= 1) AND (drain_timeout_secs <= 600))),
    CONSTRAINT llm_runtime_settings_id_check CHECK ((id = true)),
    CONSTRAINT llm_runtime_settings_idle_unload_secs_check CHECK (((idle_unload_secs >= 0) AND (idle_unload_secs <= 86400)))
);

CREATE TABLE public.llm_runtime_versions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    engine character varying(50) NOT NULL,
    version character varying(100) NOT NULL,
    platform character varying(50) NOT NULL,
    arch character varying(50) NOT NULL,
    backend character varying(50) NOT NULL,
    binary_path text NOT NULL,
    is_system_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_runtime_versions_engine_check CHECK ((engine IN ('llamacpp', 'mistralrs')))
);

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_model_id_key UNIQUE (model_id);

ALTER TABLE ONLY public.llm_runtime_instances
    ADD CONSTRAINT llm_runtime_instances_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_runtime_settings
    ADD CONSTRAINT llm_runtime_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_runtime_versions
    ADD CONSTRAINT llm_runtime_versions_engine_version_platform_arch_backend_key UNIQUE (engine, version, platform, arch, backend);

ALTER TABLE ONLY public.llm_runtime_versions
    ADD CONSTRAINT llm_runtime_versions_pkey PRIMARY KEY (id);

CREATE INDEX idx_llm_runtime_instances_last_used ON public.llm_runtime_instances USING btree (last_used_at) WHERE ((status)::text = 'running'::text);

CREATE INDEX idx_llm_runtime_instances_state ON public.llm_runtime_instances USING btree (state);

CREATE INDEX idx_runtime_instances_provider ON public.llm_runtime_instances USING btree (provider_id);

CREATE INDEX idx_runtime_instances_status ON public.llm_runtime_instances USING btree (status);

CREATE INDEX idx_runtime_instances_version ON public.llm_runtime_instances USING btree (runtime_version_id);

CREATE INDEX idx_runtime_versions_default ON public.llm_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true);

CREATE INDEX idx_runtime_versions_engine ON public.llm_runtime_versions USING btree (engine);
