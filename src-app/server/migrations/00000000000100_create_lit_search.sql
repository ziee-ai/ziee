-- Live literature search & screening (`lit_search` built-in MCP server).
--
-- Three tables, mirroring the web_search conventions (migration 97):
--   1. `lit_search_settings` — singleton deployment-wide config (master enable,
--      the UNION set of active connectors, caps, completeness toggle). Singleton
--      via `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)`.
--   2. `lit_search_connectors` — one row per CONFIGURED source. The *set* of
--      supported connectors lives in Rust (the LitConnector catalog), NOT here;
--      this table stores `{api_key, config}` keyed by the registry name. Adding
--      a source (e.g. OpenAlex later) is a code-only change, no migration.
--      `enabled_connectors` entries are validated against the catalog in Rust,
--      so there is deliberately no DB CHECK on them.
--   3. `lit_fulltext_cache` — the index for the on-disk full-text cache (the
--      blobs live at <app_data>/lit-cache/blobs/<content_hash>.txt). Maps any of
--      DOI/PMID/PMCID/arXiv → a content_hash (the blob) + provenance, with LRU
--      bookkeeping. Negative-cache rows (not_open_access/not_found) carry no hash.
--
-- Secrets use the dual-column pattern (api_key plaintext dev/legacy +
-- api_key_encrypted) like llm_providers / web_search.

CREATE TABLE lit_search_settings (
    id                            BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- Master admin switch. The built-in MCP server row is ALWAYS registered; the
    -- chat extension only attaches the tools when this is true. Connectors all
    -- work keyless, so no provider-config gate is needed (CORE self-skips when
    -- enabled-but-unkeyed).
    enabled                       BOOLEAN NOT NULL DEFAULT TRUE,

    -- UNION set of active connector registry keys (NOT an ordered fallback chain
    -- — every enabled+configured connector is queried and the results merged).
    -- Defaults to the five keyless sources; `core` is in the catalog but NOT
    -- default-on because its (free) key is mandatory. Validated in Rust against
    -- the LitConnector catalog (no DB CHECK, so new sources need no migration).
    enabled_connectors            TEXT[]  NOT NULL
                                  DEFAULT ARRAY['europepmc', 'crossref', 'semanticscholar', 'pubmed', 'arxiv'],

    -- Caps.
    max_results                   INTEGER NOT NULL DEFAULT 25,  -- deduped records per search
    per_source_limit              INTEGER NOT NULL DEFAULT 50,  -- raw hits fetched per source
    request_timeout_secs          INTEGER NOT NULL DEFAULT 30,

    -- Completeness/saturation estimate (heuristic — never a recall %). Shipped on.
    completeness_estimate_enabled BOOLEAN NOT NULL DEFAULT TRUE,

    created_at                    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT lit_max_results_range      CHECK (max_results          BETWEEN 1 AND 200),
    CONSTRAINT lit_per_source_limit_range CHECK (per_source_limit     BETWEEN 1 AND 100),
    CONSTRAINT lit_timeout_range          CHECK (request_timeout_secs BETWEEN 1 AND 120)
);

COMMENT ON TABLE lit_search_settings IS
    'Singleton deployment-wide lit_search config (enable + active connectors + caps + completeness toggle).';

INSERT INTO lit_search_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE lit_search_connectors (
    -- Registry key: 'europepmc', 'crossref', 'semanticscholar', 'pubmed',
    -- 'arxiv', 'core', and any source added later in code.
    connector         TEXT PRIMARY KEY,

    -- Dual-column secret (mirrors web_search): plaintext is dev/legacy fallback;
    -- prod writes encrypted-only. Holds e.g. the NCBI key, S2 key, Crossref Plus
    -- token, or (for core) the required CORE key.
    api_key           TEXT,
    api_key_encrypted BYTEA,

    -- Non-secret per-connector config (e.g. Crossref/OpenAlex `mailto`). Shape is
    -- owned by each connector's descriptor in Rust.
    config            JSONB NOT NULL DEFAULT '{}'::jsonb,

    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT lit_connector_nonempty CHECK (connector <> '')
);

COMMENT ON TABLE lit_search_connectors IS
    'Per-connector {api_key, config} keyed by the Rust LitConnector registry name. New connectors = code-only, no migration.';

CREATE TABLE lit_fulltext_cache (
    id               BIGSERIAL PRIMARY KEY,

    -- At least one identifier must be present (alias lookup keys). A request by
    -- any of these resolves to this row.
    doi              TEXT,
    pmid             TEXT,
    pmcid            TEXT,
    arxiv_id         TEXT,

    -- sha256 of the extracted text → the on-disk blob filename. NULL for
    -- negative-cache rows (not_open_access / not_found).
    content_hash     TEXT,

    -- 'full_text' | 'not_open_access' | 'not_found'
    status           TEXT NOT NULL,
    -- Resolver that produced it: europepmc / unpaywall / core / arxiv / s2.
    source           TEXT,
    license          TEXT,
    -- Preprint version (e.g. arXiv v2), when applicable — drives version-keyed
    -- freshness for preprints.
    version          TEXT,
    byte_size        BIGINT NOT NULL DEFAULT 0,

    fetched_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT lit_cache_has_id CHECK (
        doi IS NOT NULL OR pmid IS NOT NULL OR pmcid IS NOT NULL OR arxiv_id IS NOT NULL
    )
);

-- One cache row per identifier (partial unique — only when the id is present).
CREATE UNIQUE INDEX lit_fulltext_cache_doi   ON lit_fulltext_cache (doi)      WHERE doi      IS NOT NULL;
CREATE UNIQUE INDEX lit_fulltext_cache_pmid  ON lit_fulltext_cache (pmid)     WHERE pmid     IS NOT NULL;
CREATE UNIQUE INDEX lit_fulltext_cache_pmcid ON lit_fulltext_cache (pmcid)    WHERE pmcid    IS NOT NULL;
CREATE UNIQUE INDEX lit_fulltext_cache_arxiv ON lit_fulltext_cache (arxiv_id) WHERE arxiv_id IS NOT NULL;
-- LRU eviction scan.
CREATE INDEX lit_fulltext_cache_lru ON lit_fulltext_cache (last_accessed_at);

COMMENT ON TABLE lit_fulltext_cache IS
    'Index for the shared on-disk full-text cache: any id -> content_hash (blob) + provenance + LRU. Deployment-wide; public OA content only.';

-- Admin perms (lit_search::admin::read / lit_search::admin::manage) are held by
-- the Administrators group `*` wildcard — no grant needed. The user-facing
-- lit_search::use perm is granted to the Users group in migration 101.
