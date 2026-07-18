-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- web_search module tables + indexes + triggers.

CREATE TABLE public.user_web_search_provider_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    provider text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.web_search_providers (
    provider text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT provider_nonempty CHECK ((provider <> ''::text))
);

CREATE TABLE public.web_search_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    provider_chain text[] DEFAULT ARRAY['searxng'::text, 'brave'::text] NOT NULL,
    max_results integer DEFAULT 5 NOT NULL,
    fetch_max_bytes bigint DEFAULT 5242880 NOT NULL,
    fetch_max_chars integer DEFAULT 40000 NOT NULL,
    request_timeout_secs integer DEFAULT 20 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fetch_max_bytes_range CHECK (((fetch_max_bytes >= 65536) AND (fetch_max_bytes <= 104857600))),
    CONSTRAINT fetch_max_chars_range CHECK (((fetch_max_chars >= 1000) AND (fetch_max_chars <= 500000))),
    CONSTRAINT max_results_range CHECK (((max_results >= 1) AND (max_results <= 20))),
    CONSTRAINT request_timeout_secs_range CHECK (((request_timeout_secs >= 1) AND (request_timeout_secs <= 120))),
    CONSTRAINT web_search_settings_id_check CHECK ((id = true))
);

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_web_search_provider_keys
    ADD CONSTRAINT user_web_search_provider_keys_user_id_provider_key UNIQUE (user_id, provider);

ALTER TABLE ONLY public.web_search_providers
    ADD CONSTRAINT web_search_providers_pkey PRIMARY KEY (provider);

ALTER TABLE ONLY public.web_search_settings
    ADD CONSTRAINT web_search_settings_pkey PRIMARY KEY (id);
