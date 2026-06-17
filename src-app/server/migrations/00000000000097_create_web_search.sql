-- Web search + page fetch (`web_search` built-in MCP server).
--
-- Two tables:
--   1. `web_search_settings` — singleton deployment-wide config (master
--      enable, the active search provider, and provider-agnostic caps for
--      result count + page-fetch size/time). Singleton enforced via
--      `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)` (mirrors
--      code_sandbox_settings, migration 41).
--   2. `web_search_providers` — one row per CONFIGURED engine. The *set* of
--      supported engines is defined in Rust (the SearchProvider registry),
--      NOT here: this table just stores `{api_key, config}` keyed by the
--      registry's provider name. Adding Tavily/Exa/Google-CSE/Bing later is
--      therefore a code-only change (new trait impl + registry entry) with
--      NO migration. `provider_chain` entries are validated against the
--      registry in Rust, so there is deliberately no DB CHECK on them.
--
-- Secrets use the same dual-column pattern as llm_providers (migration 43):
-- `api_key` (legacy/dev plaintext, stays NULL in prod) + `api_key_encrypted`
-- (pgp_sym_encrypt via common::secret). API responses redact via SecretView.

CREATE TABLE web_search_settings (
    id                     BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- Master admin switch. The built-in MCP server row is ALWAYS registered;
    -- the chat extension only attaches the tools when this is true AND at
    -- least one provider in the chain is configured. Admins can hard-disable
    -- per deployment.
    enabled                BOOLEAN NOT NULL DEFAULT TRUE,

    -- Ordered fallback chain of registry keys. A search tries each entry in
    -- order, advancing to the next ONLY on error/timeout/quota — a successful
    -- (even empty) result is returned as-is. Entries are validated in Rust
    -- against the SearchProvider catalog (no DB CHECK, so new providers need
    -- no migration). Unconfigured entries are skipped at dispatch time.
    provider_chain         TEXT[]  NOT NULL DEFAULT ARRAY['searxng', 'brave'],

    -- Provider-agnostic caps.
    max_results            INTEGER NOT NULL DEFAULT 5,        -- per search call
    fetch_max_bytes        BIGINT  NOT NULL DEFAULT 5242880,  -- 5 MiB download cap
    fetch_max_chars        INTEGER NOT NULL DEFAULT 40000,    -- chars handed to the model
    request_timeout_secs   INTEGER NOT NULL DEFAULT 20,

    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Defense-in-depth range guards; the handler validates first for clearer
    -- errors, the DB is the last line.
    CONSTRAINT max_results_range          CHECK (max_results          BETWEEN 1 AND 20),
    CONSTRAINT fetch_max_bytes_range      CHECK (fetch_max_bytes      BETWEEN 65536 AND 104857600), -- 64 KiB .. 100 MiB
    CONSTRAINT fetch_max_chars_range      CHECK (fetch_max_chars      BETWEEN 1000 AND 500000),
    CONSTRAINT request_timeout_secs_range CHECK (request_timeout_secs BETWEEN 1 AND 120)
);

COMMENT ON TABLE web_search_settings IS
    'Singleton deployment-wide web_search config (enable + active provider + caps).';

INSERT INTO web_search_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE web_search_providers (
    -- Registry key: 'searxng', 'brave', and any engine added later in code.
    provider          TEXT PRIMARY KEY,

    -- Dual-column secret (mirrors llm_providers): plaintext is dev/legacy
    -- fallback only; prod writes encrypted-only.
    api_key           TEXT,
    api_key_encrypted BYTEA,

    -- Non-secret per-provider config (e.g. SearXNG `base_url`, Google CSE
    -- `cx`). Shape is owned by each provider's descriptor in Rust.
    config            JSONB NOT NULL DEFAULT '{}'::jsonb,

    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT provider_nonempty CHECK (provider <> '')
);

COMMENT ON TABLE web_search_providers IS
    'Per-engine {api_key, config} keyed by the Rust SearchProvider registry name. New engines = code-only, no migration.';

-- No seed rows: providers are admin-configured. The registry defines what is
-- selectable; this table records only what has actually been configured.

-- Admin perms (web_search::admin::read / web_search::admin::manage) are held
-- by the Administrators group's `*` wildcard (migration 1) — no grant needed.
-- The user-facing web_search::use perm is granted to the Users group in
-- migration 97.
