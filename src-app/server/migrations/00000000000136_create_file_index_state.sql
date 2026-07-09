-- Per-file RAG indexing lifecycle state — the source of truth the knowledge-base
-- UI reads for per-document status, and the source that emits the live
-- `sync:file_index_state` stream.
--
-- Chunk counts alone CANNOT distinguish these states: a file with zero
-- `file_chunks` may be pending, actively indexing, failed, or genuinely
-- text-less (scanned/image PDF). `file_rag`'s ingest path writes this row at
-- each transition (the old warn!-only failure becomes `failed`; the no-text
-- early-return becomes `no_text`).
--
-- Owner of the row is the file; shared across file_rag consumers (files_mcp /
-- project / knowledge_base). ON DELETE CASCADE on file_id auto-cleans it.

CREATE TABLE file_index_state (
    file_id     UUID PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','indexing','indexed','failed','no_text')),
    error       TEXT,                       -- populated only for `failed`
    chunk_count INTEGER NOT NULL DEFAULT 0,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_file_index_state_user ON file_index_state (user_id);
CREATE INDEX idx_file_index_state_status ON file_index_state (status);
