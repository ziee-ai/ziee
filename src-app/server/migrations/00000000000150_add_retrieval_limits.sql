-- Promote previously-hardcoded retrieval/limit constants to admin-configurable
-- settings on the shared Document-RAG admin row (the surface the Knowledge Base
-- module reuses). Defaults preserve the prior compiled-in behaviour exactly:
--   kb_max_documents      2000  (per-KB document cap; was KB_MAX_DOCUMENTS)
--   search_max_hit_chars  2000  (max chars per returned passage; was MAX_HIT_CHARS)
--   search_snippet_chars  160   (text-channel snippet length per hit)
--   search_max_top_k      50    (hard ceiling the requested top_k is clamped to)

ALTER TABLE file_rag_admin_settings
    ADD COLUMN kb_max_documents     INTEGER  NOT NULL DEFAULT 2000
        CHECK (kb_max_documents     BETWEEN 1 AND 100000),
    ADD COLUMN search_max_hit_chars INTEGER  NOT NULL DEFAULT 2000
        CHECK (search_max_hit_chars BETWEEN 100 AND 100000),
    ADD COLUMN search_snippet_chars INTEGER  NOT NULL DEFAULT 160
        CHECK (search_snippet_chars BETWEEN 20 AND 4000),
    ADD COLUMN search_max_top_k     SMALLINT NOT NULL DEFAULT 50
        CHECK (search_max_top_k     BETWEEN 1 AND 500);
