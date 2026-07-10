-- Cross-encoder reranker for file_rag retrieval (retrieve-wide -> rerank ->
-- top-k). A new model capability (`rerank`), served self-hosted via llama.cpp
-- (`--reranking`), delivered through the hub (bge-reranker-v2-m3-gguf). Mirrors
-- the `embedding_model_id` idiom on this same settings row.
--
-- OFF by default: existing `semantic_search` (files_mcp + knowledge_base) is
-- byte-identical until an admin selects a reranker model and enables it.

ALTER TABLE file_rag_admin_settings
    ADD COLUMN reranker_model_id  UUID REFERENCES llm_models(id) ON DELETE SET NULL,
    ADD COLUMN rerank_enabled     BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN rerank_candidate_k INTEGER NOT NULL DEFAULT 30
        CHECK (rerank_candidate_k BETWEEN 1 AND 200);
