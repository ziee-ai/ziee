-- Multi-level memory scope (user / project / conversation) + a full-text
-- column for hybrid (vector ⊕ FTS) retrieval.
--
-- Additive: the `scope` column defaults to 'user' and the id columns are
-- nullable, so all existing rows and existing INSERTs keep working unchanged.
-- Uniqueness/scoping is computed at read time by the retriever; this migration
-- only adds the storage + indexes.

ALTER TABLE user_memories
    ADD COLUMN scope TEXT NOT NULL DEFAULT 'user'
        CHECK (scope IN ('user', 'project', 'conversation')),
    ADD COLUMN project_id      UUID REFERENCES projects(id)      ON DELETE CASCADE,
    ADD COLUMN conversation_id UUID REFERENCES conversations(id) ON DELETE CASCADE;

-- Scope ⇔ id-presence invariant: exactly the id matching the scope is set.
ALTER TABLE user_memories
    ADD CONSTRAINT user_memories_scope_ids_chk CHECK (
        (scope = 'user'         AND project_id IS NULL     AND conversation_id IS NULL)     OR
        (scope = 'project'      AND project_id IS NOT NULL AND conversation_id IS NULL)     OR
        (scope = 'conversation' AND project_id IS NULL     AND conversation_id IS NOT NULL)
    );

-- Scoped-recall lookup paths (partial — only the rows each query touches).
CREATE INDEX idx_user_memories_scope_project
    ON user_memories(user_id, project_id)
    WHERE scope = 'project' AND deleted_at IS NULL;

CREATE INDEX idx_user_memories_scope_conversation
    ON user_memories(user_id, conversation_id)
    WHERE scope = 'conversation' AND deleted_at IS NULL;

-- Full-text column for the FTS arm of hybrid retrieval (and the FTS-only
-- fallback when no embedding model is configured). 'simple' dictionary:
-- tokenize + lowercase, no stemming — language-agnostic for a multilingual
-- user base. Generated + STORED so it auto-maintains on insert/update with no
-- application code, and works on every row regardless of `embedding`.
ALTER TABLE user_memories
    ADD COLUMN content_tsv tsvector
    GENERATED ALWAYS AS (to_tsvector('simple', content)) STORED;

CREATE INDEX idx_user_memories_tsv ON user_memories USING GIN (content_tsv);
