-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- lit_search module tables + indexes + triggers.

CREATE TABLE public.lit_fulltext_cache (
    id bigint NOT NULL,
    doi text,
    pmid text,
    pmcid text,
    arxiv_id text,
    content_hash text,
    status text NOT NULL,
    source text,
    license text,
    version text,
    byte_size bigint DEFAULT 0 NOT NULL,
    fetched_at timestamp with time zone DEFAULT now() NOT NULL,
    last_accessed_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_cache_has_id CHECK (((doi IS NOT NULL) OR (pmid IS NOT NULL) OR (pmcid IS NOT NULL) OR (arxiv_id IS NOT NULL)))
);

CREATE SEQUENCE public.lit_fulltext_cache_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.lit_fulltext_cache_id_seq OWNED BY public.lit_fulltext_cache.id;

CREATE TABLE public.lit_search_connectors (
    connector text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_connector_nonempty CHECK ((connector <> ''::text))
);

CREATE TABLE public.lit_search_settings (
    id boolean DEFAULT true NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    enabled_connectors text[] DEFAULT ARRAY['europepmc'::text, 'crossref'::text, 'semanticscholar'::text, 'pubmed'::text, 'arxiv'::text] NOT NULL,
    max_results integer DEFAULT 25 NOT NULL,
    per_source_limit integer DEFAULT 50 NOT NULL,
    request_timeout_secs integer DEFAULT 30 NOT NULL,
    completeness_estimate_enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lit_max_results_range CHECK (((max_results >= 1) AND (max_results <= 200))),
    CONSTRAINT lit_per_source_limit_range CHECK (((per_source_limit >= 1) AND (per_source_limit <= 100))),
    CONSTRAINT lit_search_settings_id_check CHECK ((id = true)),
    CONSTRAINT lit_timeout_range CHECK (((request_timeout_secs >= 1) AND (request_timeout_secs <= 120)))
);

CREATE TABLE public.user_lit_search_connector_keys (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    connector text NOT NULL,
    api_key text,
    api_key_encrypted bytea,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

ALTER TABLE ONLY public.lit_fulltext_cache ALTER COLUMN id SET DEFAULT nextval('public.lit_fulltext_cache_id_seq'::regclass);

ALTER TABLE ONLY public.lit_fulltext_cache
    ADD CONSTRAINT lit_fulltext_cache_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.lit_search_connectors
    ADD CONSTRAINT lit_search_connectors_pkey PRIMARY KEY (connector);

ALTER TABLE ONLY public.lit_search_settings
    ADD CONSTRAINT lit_search_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_lit_search_connector_keys
    ADD CONSTRAINT user_lit_search_connector_keys_user_id_connector_key UNIQUE (user_id, connector);

CREATE UNIQUE INDEX lit_fulltext_cache_arxiv ON public.lit_fulltext_cache USING btree (arxiv_id) WHERE (arxiv_id IS NOT NULL);

CREATE UNIQUE INDEX lit_fulltext_cache_doi ON public.lit_fulltext_cache USING btree (doi) WHERE (doi IS NOT NULL);

CREATE INDEX lit_fulltext_cache_lru ON public.lit_fulltext_cache USING btree (last_accessed_at);

CREATE UNIQUE INDEX lit_fulltext_cache_pmcid ON public.lit_fulltext_cache USING btree (pmcid) WHERE (pmcid IS NOT NULL);

CREATE UNIQUE INDEX lit_fulltext_cache_pmid ON public.lit_fulltext_cache USING btree (pmid) WHERE (pmid IS NOT NULL);
