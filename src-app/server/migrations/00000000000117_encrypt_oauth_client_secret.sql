-- Encrypt mcp_server_oauth_configs.client_secret at rest using the existing
-- dual-column pattern (mirrors web_search_providers.api_key and
-- llm_repositories.auth_config_encrypted). New/updated rows store the secret in
-- client_secret_encrypted (pgcrypto pgp_sym_encrypt) and NULL the plaintext
-- column; legacy plaintext rows are resolved via resolve_optional_secret's
-- fallback until their next write re-encrypts them.
ALTER TABLE mcp_server_oauth_configs
    ADD COLUMN IF NOT EXISTS client_secret_encrypted BYTEA;

-- Encrypted-only rows store NULL in the plaintext column.
ALTER TABLE mcp_server_oauth_configs
    ALTER COLUMN client_secret DROP NOT NULL;
