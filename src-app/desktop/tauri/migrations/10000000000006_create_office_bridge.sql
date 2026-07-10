-- Office bridge (`office_bridge` built-in MCP server).
--
-- Singleton deployment-wide config for the office bridge (Word/Excel/
-- PowerPoint integration). The built-in MCP server row is ALWAYS registered
-- when the deploy-level config kill switch is on; this table holds the runtime
-- admin toggle + the fixed bridge port + connection/cert diagnostics.
--
-- Singleton enforced via `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)`
-- (mirrors web_search_settings, migration 97, and code_sandbox_settings,
-- migration 41). Schema per DEC-8.

CREATE TABLE office_bridge_settings (
    id                BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- Runtime admin toggle. Distinct from the deploy-level config kill switch
    -- (`office_bridge: { enabled: false }`) which an admin cannot re-enable.
    enabled           BOOLEAN NOT NULL DEFAULT TRUE,

    -- Fixed TCP port the bridge HTTPS+WSS listener binds (dual-stack; DEC-5).
    -- Kept static so the add-in manifest `SourceLocation` can be constant.
    port              INTEGER NOT NULL DEFAULT 44300,

    -- Last time a task pane successfully connected (diagnostics), or NULL.
    last_connected_at TIMESTAMPTZ NULL,

    -- Public fingerprint of the locally-trusted bridge cert (NOT a secret;
    -- display/diagnostics only). NULL until the cert is minted (ITEM-4).
    cert_fingerprint  TEXT NULL,

    -- Defense-in-depth range guard; the handler validates first for clearer
    -- errors, the DB is the last line.
    CONSTRAINT office_bridge_port_range CHECK (port BETWEEN 1 AND 65535)
);

COMMENT ON TABLE office_bridge_settings IS
    'Singleton deployment-wide office_bridge config (runtime enable + bridge port + connection/cert diagnostics).';

INSERT INTO office_bridge_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;

-- Admin perms (office_bridge::admin::read / office_bridge::admin::manage) are
-- held by the Administrators group's `*` wildcard (migration 1) — no grant
-- needed. The user-facing office_bridge::use perm is granted to the Users
-- group in the sibling migration 10000000000007 (this desktop crate).
