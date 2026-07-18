-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- voice module tables + indexes + triggers.

CREATE TABLE public.voice_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(50) NOT NULL,
    filename character varying(200) NOT NULL,
    source character varying(20) DEFAULT 'catalog'::character varying NOT NULL,
    source_url text,
    size_bytes bigint DEFAULT 0 NOT NULL,
    sha256 character(64),
    verified boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT voice_models_source_check CHECK ((source IN ('catalog', 'url', 'upload')))
);

CREATE TABLE public.voice_runtime_instance (
    id boolean DEFAULT true NOT NULL,
    runtime_version_id uuid,
    active_model character varying(100),
    local_port integer,
    base_url text,
    status character varying(20) DEFAULT 'stopped'::character varying NOT NULL,
    state character varying(30) DEFAULT 'stopped'::character varying NOT NULL,
    state_changed_at timestamp with time zone DEFAULT now() NOT NULL,
    restart_attempts integer DEFAULT 0 NOT NULL,
    last_failure_reason text,
    last_used_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT voice_runtime_instance_id_check CHECK ((id = true)),
    CONSTRAINT voice_runtime_instance_state_check CHECK ((state IN ('starting', 'healthy', 'unhealthy', 'crashed', 'restarting', 'failed', 'stopped'))),
    CONSTRAINT voice_runtime_instance_status_check CHECK ((status IN ('stopped', 'running')))
);

CREATE TABLE public.voice_runtime_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    model character varying(50) DEFAULT 'base'::character varying NOT NULL,
    language character varying(20) DEFAULT 'auto'::character varying NOT NULL,
    idle_unload_secs integer DEFAULT 1800 NOT NULL,
    auto_start_timeout_secs integer DEFAULT 60 NOT NULL,
    drain_timeout_secs integer DEFAULT 30 NOT NULL,
    max_clip_seconds integer DEFAULT 120 NOT NULL,
    max_upload_bytes bigint DEFAULT 33554432 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    streaming_enabled boolean DEFAULT true NOT NULL,
    stream_interval_ms integer DEFAULT 1000 NOT NULL,
    stream_max_decode_secs integer DEFAULT 30 NOT NULL,
    model_source_repo character varying(200) DEFAULT 'ggerganov/whisper.cpp'::character varying NOT NULL,
    CONSTRAINT voice_runtime_settings_auto_start_timeout_secs_check CHECK (((auto_start_timeout_secs >= 1) AND (auto_start_timeout_secs <= 600))),
    CONSTRAINT voice_runtime_settings_drain_timeout_secs_check CHECK (((drain_timeout_secs >= 1) AND (drain_timeout_secs <= 600))),
    CONSTRAINT voice_runtime_settings_id_check CHECK ((id = true)),
    CONSTRAINT voice_runtime_settings_idle_unload_secs_check CHECK (((idle_unload_secs >= 0) AND (idle_unload_secs <= 86400))),
    CONSTRAINT voice_runtime_settings_max_clip_seconds_check CHECK (((max_clip_seconds >= 1) AND (max_clip_seconds <= 3600))),
    CONSTRAINT voice_runtime_settings_max_upload_bytes_check CHECK (((max_upload_bytes >= 1024) AND (max_upload_bytes <= 67108864))),
    CONSTRAINT voice_runtime_settings_stream_interval_ms_check CHECK (((stream_interval_ms >= 300) AND (stream_interval_ms <= 10000))),
    CONSTRAINT voice_runtime_settings_stream_max_decode_secs_check CHECK (((stream_max_decode_secs >= 5) AND (stream_max_decode_secs <= 600)))
);

CREATE TABLE public.voice_runtime_versions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    version character varying(100) NOT NULL,
    platform character varying(50) NOT NULL,
    arch character varying(50) NOT NULL,
    backend character varying(50) NOT NULL,
    binary_path text NOT NULL,
    is_system_default boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.voice_models
    ADD CONSTRAINT voice_models_filename_key UNIQUE (filename);

ALTER TABLE ONLY public.voice_models
    ADD CONSTRAINT voice_models_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.voice_runtime_instance
    ADD CONSTRAINT voice_runtime_instance_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.voice_runtime_settings
    ADD CONSTRAINT voice_runtime_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.voice_runtime_versions
    ADD CONSTRAINT voice_runtime_versions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.voice_runtime_versions
    ADD CONSTRAINT voice_runtime_versions_version_platform_arch_backend_key UNIQUE (version, platform, arch, backend);

CREATE UNIQUE INDEX voice_models_one_name ON public.voice_models USING btree (name);

CREATE UNIQUE INDEX voice_runtime_versions_one_default ON public.voice_runtime_versions USING btree (is_system_default) WHERE (is_system_default = true);
