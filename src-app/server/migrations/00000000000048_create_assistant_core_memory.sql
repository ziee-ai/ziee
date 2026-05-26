-- Letta-style per-assistant always-in-context memory blocks (Phase 6).
--
-- An assistant can have small named blocks (e.g. `persona`, `human`)
-- that are unconditionally prepended to the system prompt for every
-- chat call. The block content is per-user per-assistant so each user
-- gets a personalized variant.
CREATE TABLE assistant_core_memory (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    assistant_id UUID NOT NULL REFERENCES assistants(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    block_label  TEXT NOT NULL,
    content      TEXT NOT NULL,
    char_limit   INTEGER NOT NULL DEFAULT 2000 CHECK (char_limit > 0 AND char_limit <= 50000),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (assistant_id, user_id, block_label)
);

CREATE INDEX idx_core_memory_lookup
    ON assistant_core_memory(user_id, assistant_id);
