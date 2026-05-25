-- Drop the NOT NULL constraint on the legacy plaintext api_key
-- columns so that pgcrypto-encrypted rows (which set api_key = NULL
-- and api_key_encrypted = bytea) can be written.
--
-- The full A5 wiring writes ciphertext to api_key_encrypted and leaves
-- the plaintext column NULL when secrets.storage_key is configured.
-- Without this constraint relaxation, the upsert in
-- repositories/user.rs would fail with a NOT NULL violation.
--
-- llm_providers.api_key was already nullable (migration 3); only
-- user_llm_provider_api_keys needs the change. The Repository read
-- paths use resolve_optional_secret to prefer the encrypted column,
-- falling back to plaintext for old rows. Closes
-- 06-llm-provider F-02 (Critical) — wiring step.

ALTER TABLE user_llm_provider_api_keys
    ALTER COLUMN api_key DROP NOT NULL;
