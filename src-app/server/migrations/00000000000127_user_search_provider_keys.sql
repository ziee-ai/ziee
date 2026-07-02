-- Per-user API keys for web_search providers + lit_search connectors.
--
-- Mirrors `user_llm_provider_api_keys` (migration 28): each row is one user's
-- own key for one provider/connector, resolved FIRST at search time with the
-- deployment/admin key (web_search_providers / lit_search_connectors) as the
-- fallback. Dual-column storage: `api_key_encrypted` holds the ciphertext when
-- a secrets storage key is configured, `api_key` the dev-mode plaintext
-- fallback — never both. Keys are only ever exposed back to the user in masked
-- form (first-4 + ***); the raw value is scoped to the owning user's rows.

CREATE TABLE user_web_search_provider_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  api_key TEXT,
  api_key_encrypted BYTEA,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (user_id, provider)
);

CREATE TABLE user_lit_search_connector_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  connector TEXT NOT NULL,
  api_key TEXT,
  api_key_encrypted BYTEA,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  UNIQUE (user_id, connector)
);
