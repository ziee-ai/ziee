-- Per-entry secret marking + encryption-at-rest for MCP server
-- `environment_variables` and `headers`.
--
-- The current shape stores both as JSONB string maps with no secret
-- metadata. The frontend renders them as raw JSON textareas. After
-- this migration:
--
--   environment_variables (existing)            — PLAIN values only
--   environment_variables_encrypted (new)       — {KEY: base64(pgp_sym_encrypt(value))}
--                                                   for entries the user marked secret
--   environment_variables_secret_keys (new)     — TEXT[] of which key names
--                                                   are secrets (denormalized so the
--                                                   read path can redact without
--                                                   decrypting; lets the UI render the
--                                                   right control type)
--
-- Same triple for headers.
--
-- The split-column approach mirrors the existing llm_providers
-- `api_key` + `api_key_encrypted` dual-column pattern (see
-- `src-app/server/src/common/secret.rs::resolve_optional_secret`).
-- Avoids changing the on-disk shape of the existing value maps so
-- legacy callers reading `environment_variables.GITHUB_TOKEN`
-- continue to work for any non-secret entry.
--
-- No backfill: existing rows keep their plain values in
-- `environment_variables`/`headers` with empty `*_encrypted`/
-- `*_secret_keys`. Users opt into secret-marking on next save via
-- the new form editor.

ALTER TABLE mcp_servers
    ADD COLUMN environment_variables_encrypted JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN environment_variables_secret_keys TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN headers_encrypted JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN headers_secret_keys TEXT[] NOT NULL DEFAULT '{}';
