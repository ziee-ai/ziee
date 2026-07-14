-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_model module tables + indexes + triggers.

CREATE TABLE public.download_instances (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    request_data jsonb NOT NULL,
    status character varying(50) NOT NULL,
    progress_data jsonb DEFAULT '{}'::jsonb,
    error_message text,
    started_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at timestamp with time zone,
    model_id uuid,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT download_instances_status_check CHECK ((status IN ('pending', 'downloading', 'completed', 'failed', 'cancelled')))
);

CREATE TABLE public.llm_model_files (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    model_id uuid NOT NULL,
    filename character varying(500) NOT NULL,
    file_path character varying(1000) NOT NULL,
    file_size_bytes bigint NOT NULL,
    file_type character varying(50) NOT NULL,
    upload_status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    uploaded_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_model_files_upload_status_check CHECK ((upload_status IN ('pending', 'uploading', 'completed', 'failed')))
);

CREATE TABLE public.llm_models (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider_id uuid NOT NULL,
    name character varying(255) NOT NULL,
    display_name character varying(255) NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    is_deprecated boolean DEFAULT false NOT NULL,
    is_active boolean DEFAULT false NOT NULL,
    capabilities jsonb DEFAULT '{}'::jsonb,
    parameters jsonb DEFAULT '{}'::jsonb,
    file_size_bytes bigint,
    validation_status character varying(50),
    validation_issues jsonb,
    engine_type character varying(50) DEFAULT 'mistralrs'::character varying NOT NULL,
    engine_settings jsonb,
    file_format character varying(20) DEFAULT 'safetensors'::character varying NOT NULL,
    port integer,
    pid integer,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    required_runtime_version_id uuid,
    CONSTRAINT check_engine_type CHECK ((engine_type IN ('mistralrs', 'llamacpp', 'none'))),
    CONSTRAINT check_file_format CHECK ((file_format IN ('safetensors', 'pytorch', 'gguf'))),
    CONSTRAINT llm_models_display_name_not_empty CHECK (((display_name)::text <> ''::text)),
    CONSTRAINT llm_models_validation_status_check CHECK ((validation_status IN ('pending', 'await_upload', 'downloading', 'processing', 'completed', 'failed', 'valid', 'invalid', 'error', 'validation_warning')))
);

ALTER TABLE ONLY public.download_instances
    ADD CONSTRAINT download_instances_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_model_id_filename_key UNIQUE (model_id, filename);

ALTER TABLE ONLY public.llm_model_files
    ADD CONSTRAINT llm_model_files_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_models
    ADD CONSTRAINT llm_models_provider_id_name_unique UNIQUE (provider_id, name);

CREATE INDEX idx_download_instances_created_at ON public.download_instances USING btree (created_at DESC);

CREATE INDEX idx_download_instances_provider_id ON public.download_instances USING btree (provider_id);

CREATE INDEX idx_download_instances_repository_id ON public.download_instances USING btree (repository_id);

CREATE INDEX idx_download_instances_status ON public.download_instances USING btree (status);

CREATE INDEX idx_llm_model_files_model_id ON public.llm_model_files USING btree (model_id);

CREATE INDEX idx_llm_model_files_upload_status ON public.llm_model_files USING btree (upload_status);

CREATE INDEX idx_llm_models_created_at ON public.llm_models USING btree (created_at DESC);

CREATE INDEX idx_llm_models_enabled ON public.llm_models USING btree (enabled);

CREATE INDEX idx_llm_models_engine_type ON public.llm_models USING btree (engine_type);

CREATE INDEX idx_llm_models_provider_id ON public.llm_models USING btree (provider_id);

CREATE INDEX idx_llm_models_validation_status ON public.llm_models USING btree (validation_status);

CREATE INDEX idx_models_required_runtime_version ON public.llm_models USING btree (required_runtime_version_id) WHERE (required_runtime_version_id IS NOT NULL);

CREATE UNIQUE INDEX uq_download_instances_in_progress ON public.download_instances USING btree (repository_id, provider_id, ((request_data ->> 'repository_path'::text)), ((request_data ->> 'main_filename'::text))) WHERE (status IN ('pending', 'downloading'));
