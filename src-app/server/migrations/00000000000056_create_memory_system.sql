-- Memory system: per-user fact store with vector retrieval.
--
-- Architecture per the plan in feat/memory-system:
--   - user_memories          per-user fact rows, vector-indexed
--   - user_memory_settings   per-user opt-in toggles
--   - memory_admin_settings  deployment-wide config (single row)
--
-- Defaults are privacy-first opt-in: extraction + retrieval are OFF for
-- new accounts; the admin enables memory globally (and picks an
-- embedding model) via the onboarding wizard or the Memory admin page.
--
-- Dimensions default to 768 to match nomic-embed-text-v1.5 (the
-- recommended local embedder). Switching to a different-dimension
-- embedding model is a one-shot ALTER COLUMN + re-embed-all operation
-- (see memory_admin module).
--
-- Column type is `halfvec` (fp16) not `vector` (fp32):
--   - Half the storage per row.
--   - Faster cosine search (SIMD: 2× half-floats per cycle, more
--     rows in L1/L2 cache).
--   - Doubled max indexable dim (4000 vs 2000) — important when
--     pairing with high-dim providers (Gemini 3072, OpenAI 3-large
--     3072) without falling off the index cliff.
--   - <1% recall@10 hit vs fp32 in practice (pgvector/FAISS papers).
--
-- Index is `hnsw` not `ivfflat`:
--   - >95% recall at default params; ivfflat needs `probes` tuning.
--   - Faster search at our scale (100-10k rows per user).
--   - No `lists` to size against unknown dataset growth.

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE user_memories (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content           TEXT NOT NULL,
    embedding         halfvec(768),
    embedding_model   TEXT,
    source            TEXT NOT NULL CHECK (source IN ('extraction', 'mcp_tool', 'manual')),
    source_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    importance        SMALLINT NOT NULL DEFAULT 50 CHECK (importance >= 0 AND importance <= 100),
    confidence        SMALLINT NOT NULL DEFAULT 80 CHECK (confidence >= 0 AND confidence <= 100),
    kind              TEXT NOT NULL DEFAULT 'fact' CHECK (kind IN ('preference', 'fact', 'goal', 'relationship', 'other')),
    metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_recalled_at  TIMESTAMPTZ,
    recall_count      INTEGER NOT NULL DEFAULT 0,
    deleted_at        TIMESTAMPTZ
);

CREATE INDEX idx_user_memories_user
    ON user_memories(user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_user_memories_user_updated
    ON user_memories(user_id, updated_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_user_memories_embedding
    ON user_memories USING hnsw (embedding halfvec_cosine_ops);
CREATE INDEX idx_user_memories_metadata
    ON user_memories USING GIN (metadata);

CREATE TABLE user_memory_settings (
    user_id              UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    extraction_enabled   BOOLEAN NOT NULL DEFAULT FALSE,
    retrieval_enabled    BOOLEAN NOT NULL DEFAULT FALSE,
    max_memories         INTEGER NOT NULL DEFAULT 1000 CHECK (max_memories > 0 AND max_memories <= 100000),
    retention_days       INTEGER,
    extraction_model_id  UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE memory_admin_settings (
    id                          SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    embedding_model_id          UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    embedding_dimensions        INTEGER NOT NULL DEFAULT 768 CHECK (embedding_dimensions > 0 AND embedding_dimensions <= 16000),
    default_extraction_model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    default_top_k               SMALLINT NOT NULL DEFAULT 8 CHECK (default_top_k > 0 AND default_top_k <= 100),
    cosine_threshold            REAL NOT NULL DEFAULT 0.6 CHECK (cosine_threshold >= 0.0 AND cosine_threshold <= 2.0),
    enabled                     BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO memory_admin_settings (id) VALUES (1) ON CONFLICT DO NOTHING;
