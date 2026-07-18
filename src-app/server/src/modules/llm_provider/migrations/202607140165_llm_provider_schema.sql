-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_provider module tables + indexes + triggers.

CREATE TABLE public.llm_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    provider_type character varying(50) NOT NULL,
    enabled boolean DEFAULT false NOT NULL,
    api_key text,
    base_url character varying(512),
    built_in boolean DEFAULT false NOT NULL,
    proxy_settings jsonb DEFAULT '{}'::jsonb,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deployment_config jsonb DEFAULT '{"type": "local", "binary_path": null}'::jsonb,
    default_runtime_version_id uuid,
    api_key_encrypted bytea,
    CONSTRAINT llm_providers_provider_type_check CHECK ((provider_type IN ('local', 'openai', 'anthropic', 'groq', 'gemini', 'mistral', 'deepseek', 'huggingface', 'custom', 'openrouter')))
);

CREATE TABLE public.user_group_llm_providers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    group_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.user_llm_provider_api_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    api_key text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    api_key_encrypted bytea
);

ALTER TABLE ONLY public.llm_providers
    ADD CONSTRAINT llm_providers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_group_id_provider_id_key UNIQUE (group_id, provider_id);

ALTER TABLE ONLY public.user_group_llm_providers
    ADD CONSTRAINT user_group_llm_providers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_llm_provider_api_keys
    ADD CONSTRAINT user_llm_provider_api_keys_user_id_provider_id_key UNIQUE (user_id, provider_id);

CREATE INDEX idx_llm_providers_enabled ON public.llm_providers USING btree (enabled);

CREATE INDEX idx_llm_providers_type ON public.llm_providers USING btree (provider_type);

CREATE INDEX idx_providers_default_runtime_version ON public.llm_providers USING btree (default_runtime_version_id) WHERE (default_runtime_version_id IS NOT NULL);

CREATE INDEX idx_ugp_group ON public.user_group_llm_providers USING btree (group_id);

CREATE INDEX idx_ugp_provider ON public.user_group_llm_providers USING btree (provider_id);

CREATE TRIGGER update_llm_providers_updated_at BEFORE UPDATE ON public.llm_providers FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
