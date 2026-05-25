-- Add pgcrypto extension + encrypted columns for at-rest secret storage.
-- Closes 06-llm-provider F-02 (Critical) — api_key was stored as plaintext
-- in llm_providers.api_key, exposing any DB dump / backup / read replica to
-- credential disclosure.
--
-- Strategy:
--   1. Add a bytea column alongside the existing text column.
--   2. New writes go to the bytea column (encrypted with the
--      secrets.storage_key from config).
--   3. Reads prefer the bytea column; fall back to the text column for
--      not-yet-backfilled rows.
--   4. The text column will be dropped in a follow-up migration after
--      all rows have been backfilled (one release cycle later).

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE llm_providers
    ADD COLUMN api_key_encrypted BYTEA;

-- Index name matches existing column naming convention. No index here —
-- the field is only ever fetched by row, never queried by value.

-- Same for per-user provider API keys.
ALTER TABLE user_llm_provider_api_keys
    ADD COLUMN api_key_encrypted BYTEA;

-- Same for llm_repositories.auth_config (the whole JSONB blob is
-- encrypted as one unit; the column already isn't queried by value).
ALTER TABLE llm_repositories
    ADD COLUMN auth_config_encrypted BYTEA;
