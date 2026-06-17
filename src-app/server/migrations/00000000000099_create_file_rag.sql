-- Document RAG: semantic + full-text retrieval over the extracted full text
-- of files a user uploaded to a project (knowledge bundle) or attached to a
-- conversation. Surfaced as the `files_mcp` `semantic_search` tool; each hit
-- carries (file_id, page, char span) so a citation layer can cite precisely.
--
-- Reuses the `memory` module's vector stack verbatim in spirit:
--   - file_chunks            per-file text chunks, vector + FTS indexed
--   - file_rag_admin_settings deployment-wide config (single row)
--
-- Design notes (see the feat/document-rag plan):
--   - Files are SHARED, not copied (one file_id linked to many
--     projects/conversations), so we index ONCE per file (head version),
--     keyed by file_id, and filter at query time by file_id = ANY(scope).
--     ON DELETE CASCADE on file_id makes chunk cleanup automatic.
--   - `content_tsv` is GENERATED, so FTS works the moment a chunk is inserted
--     — NO embedding model required. The vector arm activates only once an
--     admin sets `embedding_model_id`. `embedding` is therefore nullable.
--   - Default is ON (FTS from day one). This deliberately diverges from
--     memory's opt-in default: memory infers facts ABOUT the user (opt-in),
--     whereas Document RAG only makes the user's OWN uploaded files
--     searchable — expected behavior, no inference, no external egress.
--
-- Column type is `halfvec` (fp16), index is `hnsw` — same rationale as
-- migration 56 (half storage, faster cosine, 4000-dim ceiling, >95% recall).
-- `embedding_dimensions` is capped at 4000 (the HNSW halfvec ceiling) rather
-- than memory's looser <=16000 (a latent bug there: a >4000-dim model passes
-- memory's CHECK but fails CREATE INDEX).

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE file_chunks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id         UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The STORAGE key (= the head file_versions.id at index time). Storage is
    -- addressed by THIS, not file_id — the save/load_text_page trait param is
    -- misleadingly *named* `file_id` but callers pass blob_version_id. Citation
    -- re-load = load_text_page(user_id, blob_version_id, page_number).
    blob_version_id UUID NOT NULL,
    version         INTEGER NOT NULL,       -- head version number at index time (display/provenance)
    page_number     INTEGER NOT NULL,       -- 1-indexed, matches storage page files
    chunk_index     INTEGER NOT NULL,       -- 0-based order within the file
    char_start      INTEGER NOT NULL,       -- inclusive, char offset relative to the PAGE's extracted text
    char_end        INTEGER NOT NULL,       -- exclusive
    content         TEXT NOT NULL,
    embedding       halfvec(768),           -- nullable: FTS works without it; vector arm fills it in
    embedding_model TEXT,
    content_tsv     tsvector GENERATED ALWAYS AS (to_tsvector('simple', content)) STORED,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_file_chunks_file
    ON file_chunks(file_id);
CREATE INDEX idx_file_chunks_embedding
    ON file_chunks USING hnsw (embedding halfvec_cosine_ops);
CREATE INDEX idx_file_chunks_tsv
    ON file_chunks USING GIN (content_tsv);

CREATE TABLE file_rag_admin_settings (
    id                       SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    -- ON by default: FTS from day one (vector arm needs embedding_model_id).
    enabled                  BOOLEAN NOT NULL DEFAULT TRUE,
    -- Option B: file_rag owns its own embedder, independent of memory's.
    embedding_model_id       UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    embedding_dimensions     INTEGER NOT NULL DEFAULT 768
        CHECK (embedding_dimensions > 0 AND embedding_dimensions <= 4000),
    -- Chunking
    chunk_chars              INTEGER NOT NULL DEFAULT 1200
        CHECK (chunk_chars BETWEEN 200 AND 8000),
    chunk_overlap_chars      INTEGER NOT NULL DEFAULT 200
        CHECK (chunk_overlap_chars >= 0 AND chunk_overlap_chars < chunk_chars),
    max_chunks_per_file      INTEGER NOT NULL DEFAULT 5000
        CHECK (max_chunks_per_file > 0),
    -- Retrieval tuning (mirror memory_admin_settings)
    default_top_k            SMALLINT NOT NULL DEFAULT 8
        CHECK (default_top_k > 0 AND default_top_k <= 50),
    cosine_threshold         REAL NOT NULL DEFAULT 0.6
        CHECK (cosine_threshold >= 0.0 AND cosine_threshold <= 2.0),
    semantic_enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    fts_enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    fts_dictionary           TEXT NOT NULL DEFAULT 'simple'
        CHECK (fts_dictionary IN ('simple','english','french','german',
                                  'spanish','italian','portuguese','russian',
                                  'dutch','norwegian','swedish','danish',
                                  'finnish','hungarian','turkish')),
    fts_rrf_k                INTEGER NOT NULL DEFAULT 60
        CHECK (fts_rrf_k BETWEEN 1 AND 1000),
    fts_candidate_multiplier INTEGER NOT NULL DEFAULT 4
        CHECK (fts_candidate_multiplier BETWEEN 1 AND 20),
    fts_min_rank             REAL NOT NULL DEFAULT 0.0
        CHECK (fts_min_rank BETWEEN 0.0 AND 1.0),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO file_rag_admin_settings (id) VALUES (1) ON CONFLICT DO NOTHING;
