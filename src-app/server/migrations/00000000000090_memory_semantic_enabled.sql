-- Memory: semantic-search on/off toggle (mirror of fts_enabled from
-- migration 89). Previously the vector arm was implicitly enabled
-- whenever an embedding model was configured — that conflated "is a
-- model picked" with "should we run vector recall," and meant admins
-- couldn't disable semantic recall without clearing the model picker.
-- The retriever now consults this flag explicitly:
--
--   vector_arm = semantic_enabled AND embedding_model_id IS NOT NULL
--
-- Default TRUE preserves prior behavior for existing deployments
-- where a model is already set.

ALTER TABLE memory_admin_settings
    ADD COLUMN semantic_enabled BOOLEAN NOT NULL DEFAULT TRUE;
