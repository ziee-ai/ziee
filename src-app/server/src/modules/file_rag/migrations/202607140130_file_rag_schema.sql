-- Squashed module baseline (MIGRATE-squash / N3.1 / N7).
-- file_rag module tables + indexes + triggers.

CREATE TABLE public.file_chunks (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    file_id uuid NOT NULL,
    user_id uuid NOT NULL,
    blob_version_id uuid NOT NULL,
    version integer NOT NULL,
    page_number integer NOT NULL,
    chunk_index integer NOT NULL,
    char_start integer NOT NULL,
    char_end integer NOT NULL,
    content text NOT NULL,
    embedding public.halfvec(768),
    embedding_model text,
    content_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple'::regconfig, content)) STORED,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.file_index_state (
    file_id uuid NOT NULL,
    user_id uuid NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    error text,
    chunk_count integer DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT file_index_state_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'indexing'::text, 'indexed'::text, 'failed'::text, 'no_text'::text])))
);

CREATE TABLE public.file_rag_admin_settings (
    id smallint DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    embedding_model_id uuid,
    embedding_dimensions integer DEFAULT 768 NOT NULL,
    chunk_chars integer DEFAULT 1200 NOT NULL,
    chunk_overlap_chars integer DEFAULT 200 NOT NULL,
    max_chunks_per_file integer DEFAULT 5000 NOT NULL,
    default_top_k smallint DEFAULT 8 NOT NULL,
    cosine_threshold real DEFAULT 0.6 NOT NULL,
    semantic_enabled boolean DEFAULT true NOT NULL,
    fts_enabled boolean DEFAULT true NOT NULL,
    fts_dictionary text DEFAULT 'simple'::text NOT NULL,
    fts_rrf_k integer DEFAULT 60 NOT NULL,
    fts_candidate_multiplier integer DEFAULT 4 NOT NULL,
    fts_min_rank real DEFAULT 0.0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    reranker_model_id uuid,
    rerank_enabled boolean DEFAULT false NOT NULL,
    rerank_candidate_k integer DEFAULT 30 NOT NULL,
    kb_max_documents integer DEFAULT 2000 NOT NULL,
    search_max_hit_chars integer DEFAULT 2000 NOT NULL,
    search_snippet_chars integer DEFAULT 160 NOT NULL,
    search_max_top_k smallint DEFAULT 50 NOT NULL,
    CONSTRAINT file_rag_admin_settings_check CHECK (((chunk_overlap_chars >= 0) AND (chunk_overlap_chars < chunk_chars))),
    CONSTRAINT file_rag_admin_settings_chunk_chars_check CHECK (((chunk_chars >= 200) AND (chunk_chars <= 8000))),
    CONSTRAINT file_rag_admin_settings_cosine_threshold_check CHECK (((cosine_threshold >= (0.0)::double precision) AND (cosine_threshold <= (2.0)::double precision))),
    CONSTRAINT file_rag_admin_settings_default_top_k_check CHECK (((default_top_k > 0) AND (default_top_k <= 50))),
    CONSTRAINT file_rag_admin_settings_embedding_dimensions_check CHECK (((embedding_dimensions > 0) AND (embedding_dimensions <= 4000))),
    CONSTRAINT file_rag_admin_settings_fts_candidate_multiplier_check CHECK (((fts_candidate_multiplier >= 1) AND (fts_candidate_multiplier <= 20))),
    CONSTRAINT file_rag_admin_settings_fts_dictionary_check CHECK ((fts_dictionary = ANY (ARRAY['simple'::text, 'english'::text, 'french'::text, 'german'::text, 'spanish'::text, 'italian'::text, 'portuguese'::text, 'russian'::text, 'dutch'::text, 'norwegian'::text, 'swedish'::text, 'danish'::text, 'finnish'::text, 'hungarian'::text, 'turkish'::text]))),
    CONSTRAINT file_rag_admin_settings_fts_min_rank_check CHECK (((fts_min_rank >= (0.0)::double precision) AND (fts_min_rank <= (1.0)::double precision))),
    CONSTRAINT file_rag_admin_settings_fts_rrf_k_check CHECK (((fts_rrf_k >= 1) AND (fts_rrf_k <= 1000))),
    CONSTRAINT file_rag_admin_settings_id_check CHECK ((id = 1)),
    CONSTRAINT file_rag_admin_settings_kb_max_documents_check CHECK (((kb_max_documents >= 1) AND (kb_max_documents <= 100000))),
    CONSTRAINT file_rag_admin_settings_max_chunks_per_file_check CHECK ((max_chunks_per_file > 0)),
    CONSTRAINT file_rag_admin_settings_rerank_candidate_k_check CHECK (((rerank_candidate_k >= 1) AND (rerank_candidate_k <= 200))),
    CONSTRAINT file_rag_admin_settings_search_max_hit_chars_check CHECK (((search_max_hit_chars >= 100) AND (search_max_hit_chars <= 100000))),
    CONSTRAINT file_rag_admin_settings_search_max_top_k_check CHECK (((search_max_top_k >= 1) AND (search_max_top_k <= 500))),
    CONSTRAINT file_rag_admin_settings_search_snippet_chars_check CHECK (((search_snippet_chars >= 20) AND (search_snippet_chars <= 4000)))
);

ALTER TABLE ONLY public.file_chunks
    ADD CONSTRAINT file_chunks_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.file_index_state
    ADD CONSTRAINT file_index_state_pkey PRIMARY KEY (file_id);

ALTER TABLE ONLY public.file_rag_admin_settings
    ADD CONSTRAINT file_rag_admin_settings_pkey PRIMARY KEY (id);

CREATE INDEX idx_file_chunks_embedding ON public.file_chunks USING hnsw (embedding public.halfvec_cosine_ops);

CREATE INDEX idx_file_chunks_file ON public.file_chunks USING btree (file_id);

CREATE INDEX idx_file_chunks_tsv ON public.file_chunks USING gin (content_tsv);

CREATE INDEX idx_file_index_state_status ON public.file_index_state USING btree (status);

CREATE INDEX idx_file_index_state_user ON public.file_index_state USING btree (user_id);
