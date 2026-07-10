-- Knowledge bases: a user-owned, standalone-reusable collection of files the
-- agent retrieves from via the `search_knowledge` MCP tool (RAG at scale).
--
-- A KB is a named SET of file_ids; chunks/embeddings live in the shared
-- `file_chunks` table (migration 99), keyed by file_id. So a file can belong to
-- many KBs (and to the conversation-files path) simultaneously; removing a file
-- from a KB or deleting a KB deletes ONLY the join row, never the shared
-- `file_chunks` (which has no kb_id) — verified data-integrity property.
--
-- No denormalized document_count: it is derived at read (COUNT(*)) because an
-- external file delete cascades the join row without an app-level decrement.
--
-- Owner-scoped: retrieval is filtered by user_id both at the KB scope-resolve
-- layer and by `semantic_search`'s own user_id guard.

CREATE TABLE knowledge_bases (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Per-user unique name (case-insensitive), like the citations/library idioms.
CREATE UNIQUE INDEX idx_knowledge_bases_user_name
    ON knowledge_bases (user_id, lower(name));
CREATE INDEX idx_knowledge_bases_user
    ON knowledge_bases (user_id);

-- M:N membership (mirrors project_files). Composite PK prevents attaching the
-- same file_id twice; file_id index supports "which KBs is this file in".
CREATE TABLE knowledge_base_documents (
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    file_id           UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    added_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (knowledge_base_id, file_id)
);
CREATE INDEX idx_knowledge_base_documents_file
    ON knowledge_base_documents (file_id);

-- Attach a KB to a conversation (direct scope for search_knowledge).
CREATE TABLE conversation_knowledge_bases (
    conversation_id   UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    added_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (conversation_id, knowledge_base_id)
);
CREATE INDEX idx_conversation_knowledge_bases_kb
    ON conversation_knowledge_bases (knowledge_base_id);

-- Attach a KB to a project (read-through: a conversation inherits its project's
-- KBs at query time, unioned with its direct attachments).
CREATE TABLE project_knowledge_bases (
    project_id        UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    added_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, knowledge_base_id)
);
CREATE INDEX idx_project_knowledge_bases_kb
    ON project_knowledge_bases (knowledge_base_id);
