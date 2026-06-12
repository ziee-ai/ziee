-- Expose the four hardcoded FTS-side constants from the chat-extension
-- retriever (`websearch_to_tsquery` dictionary, RRF k=60, hybrid candidate
-- multiplier 4x, ts_rank_cd floor 0.0) plus a kill switch (`fts_enabled`)
-- as admin-tunable columns.
--
-- Defaults match today's hardcoded behavior byte-for-byte, so the
-- migration is observably a no-op until the admin changes something.
--
-- Dictionary is a special case: it's baked into `user_memories.content_tsv`'s
-- GENERATED ALWAYS expression and can't change in place. The PUT-settings
-- handler returns 409 FTS_REBUILD_REQUIRED on a dictionary change; the
-- caller must hit POST /api/memory/admin/fts/rebuild to swap the column.
-- The two `fts_rebuild_*` timestamps below surface that operation's state
-- so the admin UI's rebuild-status pill can show progress.

ALTER TABLE memory_admin_settings
    ADD COLUMN fts_dictionary           TEXT    NOT NULL DEFAULT 'simple'
        CHECK (fts_dictionary IN ('simple','english','french','german',
                                  'spanish','italian','portuguese','russian',
                                  'dutch','norwegian','swedish','danish',
                                  'finnish','hungarian','turkish')),
    ADD COLUMN fts_enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN fts_rrf_k                INTEGER NOT NULL DEFAULT 60
        CHECK (fts_rrf_k BETWEEN 1 AND 1000),
    ADD COLUMN fts_candidate_multiplier INTEGER NOT NULL DEFAULT 4
        CHECK (fts_candidate_multiplier BETWEEN 1 AND 20),
    ADD COLUMN fts_min_rank             REAL    NOT NULL DEFAULT 0.0
        CHECK (fts_min_rank BETWEEN 0.0 AND 1.0),
    ADD COLUMN fts_rebuild_started_at   TIMESTAMPTZ,
    ADD COLUMN fts_rebuild_completed_at TIMESTAMPTZ;
