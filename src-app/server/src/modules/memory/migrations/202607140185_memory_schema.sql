-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- memory module tables + indexes + triggers.

CREATE TABLE public.conversation_memory_settings (
    conversation_id uuid NOT NULL,
    memory_mode text NOT NULL,
    CONSTRAINT conversation_memory_settings_memory_mode_check CHECK ((memory_mode = ANY (ARRAY['inherit'::text, 'on'::text, 'off'::text])))
);

CREATE TABLE public.memory_admin_settings (
    id smallint DEFAULT 1 NOT NULL,
    embedding_model_id uuid,
    embedding_dimensions integer DEFAULT 768 NOT NULL,
    default_extraction_model_id uuid,
    default_top_k smallint DEFAULT 8 NOT NULL,
    cosine_threshold real DEFAULT 0.6 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    soft_delete_grace_days integer DEFAULT 30 NOT NULL,
    daily_extraction_quota integer DEFAULT 200 NOT NULL,
    fts_dictionary text DEFAULT 'simple'::text NOT NULL,
    fts_enabled boolean DEFAULT true NOT NULL,
    fts_rrf_k integer DEFAULT 60 NOT NULL,
    fts_candidate_multiplier integer DEFAULT 4 NOT NULL,
    fts_min_rank real DEFAULT 0.0 NOT NULL,
    fts_rebuild_started_at timestamp with time zone,
    fts_rebuild_completed_at timestamp with time zone,
    semantic_enabled boolean DEFAULT true NOT NULL,
    CONSTRAINT memory_admin_settings_cosine_threshold_check CHECK (((cosine_threshold >= (0.0)::double precision) AND (cosine_threshold <= (2.0)::double precision))),
    CONSTRAINT memory_admin_settings_daily_extraction_quota_check CHECK (((daily_extraction_quota >= 1) AND (daily_extraction_quota <= 10000))),
    CONSTRAINT memory_admin_settings_default_top_k_check CHECK (((default_top_k > 0) AND (default_top_k <= 100))),
    CONSTRAINT memory_admin_settings_embedding_dimensions_check CHECK (((embedding_dimensions > 0) AND (embedding_dimensions <= 16000))),
    CONSTRAINT memory_admin_settings_fts_candidate_multiplier_check CHECK (((fts_candidate_multiplier >= 1) AND (fts_candidate_multiplier <= 20))),
    CONSTRAINT memory_admin_settings_fts_dictionary_check CHECK ((fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))),
    CONSTRAINT memory_admin_settings_fts_min_rank_check CHECK (((fts_min_rank >= (0.0)::double precision) AND (fts_min_rank <= (1.0)::double precision))),
    CONSTRAINT memory_admin_settings_fts_rrf_k_check CHECK (((fts_rrf_k >= 1) AND (fts_rrf_k <= 1000))),
    CONSTRAINT memory_admin_settings_id_check CHECK ((id = 1)),
    CONSTRAINT memory_admin_settings_soft_delete_grace_days_check CHECK (((soft_delete_grace_days >= 1) AND (soft_delete_grace_days <= 365)))
);

CREATE TABLE public.memory_audit_log (
    id bigint NOT NULL,
    user_id uuid NOT NULL,
    memory_id uuid,
    op text NOT NULL,
    source text NOT NULL,
    content_snapshot text,
    actor_kind text DEFAULT 'user'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT memory_audit_log_actor_kind_check CHECK ((actor_kind = ANY (ARRAY['user'::text, 'assistant'::text, 'admin'::text, 'system'::text]))),
    CONSTRAINT memory_audit_log_op_check CHECK ((op = ANY (ARRAY['ADD'::text, 'UPDATE'::text, 'DELETE'::text, 'BULK_DELETE'::text]))),
    CONSTRAINT memory_audit_log_source_check CHECK ((source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text, 'admin'::text])))
);

CREATE SEQUENCE public.memory_audit_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.memory_audit_log_id_seq OWNED BY public.memory_audit_log.id;

CREATE TABLE public.user_memories (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    content text NOT NULL,
    embedding public.halfvec(768),
    embedding_model text,
    source text NOT NULL,
    source_message_id uuid,
    importance smallint DEFAULT 50 NOT NULL,
    confidence smallint DEFAULT 80 NOT NULL,
    kind text DEFAULT 'fact'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_recalled_at timestamp with time zone,
    recall_count integer DEFAULT 0 NOT NULL,
    deleted_at timestamp with time zone,
    scope text DEFAULT 'user'::text NOT NULL,
    project_id uuid,
    conversation_id uuid,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, content)) STORED,
    CONSTRAINT user_memories_confidence_check CHECK (((confidence >= 0) AND (confidence <= 100))),
    CONSTRAINT user_memories_importance_check CHECK (((importance >= 0) AND (importance <= 100))),
    CONSTRAINT user_memories_kind_check CHECK ((kind = ANY (ARRAY['preference'::text, 'fact'::text, 'goal'::text, 'relationship'::text, 'other'::text]))),
    CONSTRAINT user_memories_scope_check CHECK ((scope = ANY (ARRAY['user'::text, 'project'::text, 'conversation'::text]))),
    CONSTRAINT user_memories_scope_ids_chk CHECK ((((scope = 'user'::text) AND (project_id IS NULL) AND (conversation_id IS NULL)) OR ((scope = 'project'::text) AND (project_id IS NOT NULL) AND (conversation_id IS NULL)) OR ((scope = 'conversation'::text) AND (project_id IS NULL) AND (conversation_id IS NOT NULL)))),
    CONSTRAINT user_memories_source_check CHECK ((source = ANY (ARRAY['extraction'::text, 'mcp_tool'::text, 'manual'::text])))
);

CREATE TABLE public.user_memory_settings (
    user_id uuid NOT NULL,
    extraction_enabled boolean DEFAULT false NOT NULL,
    retrieval_enabled boolean DEFAULT false NOT NULL,
    max_memories integer DEFAULT 1000 NOT NULL,
    retention_days integer,
    extraction_model_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT user_memory_settings_max_memories_check CHECK (((max_memories > 0) AND (max_memories <= 100000)))
);

ALTER TABLE ONLY public.memory_audit_log ALTER COLUMN id SET DEFAULT nextval('public.memory_audit_log_id_seq'::regclass);

ALTER TABLE ONLY public.conversation_memory_settings
    ADD CONSTRAINT conversation_memory_settings_pkey PRIMARY KEY (conversation_id);

ALTER TABLE ONLY public.memory_admin_settings
    ADD CONSTRAINT memory_admin_settings_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.memory_audit_log
    ADD CONSTRAINT memory_audit_log_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_memories
    ADD CONSTRAINT user_memories_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_memory_settings
    ADD CONSTRAINT user_memory_settings_pkey PRIMARY KEY (user_id);

CREATE INDEX idx_memory_audit_log_memory ON public.memory_audit_log USING btree (memory_id) WHERE (memory_id IS NOT NULL);

CREATE INDEX idx_memory_audit_log_user_created ON public.memory_audit_log USING btree (user_id, created_at DESC);

CREATE INDEX idx_user_memories_embedding ON public.user_memories USING hnsw (embedding public.halfvec_cosine_ops);

CREATE INDEX idx_user_memories_extraction_quota ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text);

CREATE INDEX idx_user_memories_extraction_recent ON public.user_memories USING btree (user_id, created_at) WHERE (source = 'extraction'::text);

CREATE INDEX idx_user_memories_metadata ON public.user_memories USING gin (metadata);

CREATE INDEX idx_user_memories_scope_conversation ON public.user_memories USING btree (user_id, conversation_id) WHERE ((scope = 'conversation'::text) AND (deleted_at IS NULL));

CREATE INDEX idx_user_memories_scope_project ON public.user_memories USING btree (user_id, project_id) WHERE ((scope = 'project'::text) AND (deleted_at IS NULL));

CREATE INDEX idx_user_memories_scope_user ON public.user_memories USING btree (user_id) WHERE ((scope = 'user'::text) AND (deleted_at IS NULL));

CREATE INDEX idx_user_memories_tsv ON public.user_memories USING gin (content_tsv);

CREATE INDEX idx_user_memories_user ON public.user_memories USING btree (user_id) WHERE (deleted_at IS NULL);

CREATE INDEX idx_user_memories_user_updated ON public.user_memories USING btree (user_id, updated_at DESC) WHERE (deleted_at IS NULL);
