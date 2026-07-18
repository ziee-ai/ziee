-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- llm_repository module tables + indexes + triggers.

CREATE TABLE public.llm_repositories (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    url character varying(512) NOT NULL,
    auth_type character varying(50) NOT NULL,
    auth_config jsonb DEFAULT '{}'::jsonb,
    enabled boolean DEFAULT true NOT NULL,
    built_in boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    auth_config_encrypted bytea,
    last_health_check_at timestamp with time zone,
    last_health_check_status text DEFAULT 'untested'::text NOT NULL,
    last_health_check_reason text,
    CONSTRAINT llm_repositories_auth_type_check CHECK ((auth_type IN ('none', 'api_key', 'basic_auth', 'bearer_token'))),
    CONSTRAINT llm_repositories_last_health_check_status_check CHECK ((last_health_check_status = ANY (ARRAY['untested'::text, 'healthy'::text, 'unhealthy'::text])))
);

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_name_key UNIQUE (name);

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.llm_repositories
    ADD CONSTRAINT llm_repositories_url_key UNIQUE (url);

CREATE INDEX idx_llm_repositories_health_status_unhealthy ON public.llm_repositories USING btree (last_health_check_status) WHERE (last_health_check_status = 'unhealthy'::text);

CREATE TRIGGER update_llm_repositories_updated_at BEFORE UPDATE ON public.llm_repositories FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
