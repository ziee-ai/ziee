-- Host-folder mounts (feature #3, Part B) — a DESKTOP-ONLY capability.
--
-- Lets a user mount a folder from their own machine into the code sandbox
-- (read-only by default) so multi-GB/TB genomics files (BAM/FASTQ/VCF) can be
-- read in place instead of uploaded. Only possible where the sandbox runs on
-- the user's machine (desktop / self-hosted-local) — the desktop crate
-- registers a `SandboxMountProvider` against the generic server seam; a remote
-- web server registers none and the feature is physically inert there.
--
-- Two tables, both desktop-owned (the server core never names "host folders"):
--   * host_mount_policy  — singleton deployment policy (id=1).
--   * host_mounts        — per-scope (conversation XOR project) folder list.
--
-- The per-conversation row is resolved at execute_command time with
-- read-through fallback to its project's row, so editing a project's folders
-- is reflected in its existing conversations (deliberately unlike the
-- MCP-settings snapshot).

CREATE TABLE IF NOT EXISTS host_mount_policy (
    id               SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    -- Master on/off for the whole feature.
    enabled          BOOLEAN NOT NULL DEFAULT TRUE,
    -- Allowed host path prefixes. Empty array = allow any path (the common
    -- single-user desktop case). When non-empty, a folder is only mounted if
    -- its host_path starts with one of these prefixes.
    allowed_prefixes TEXT[] NOT NULL DEFAULT '{}',
    -- Read-write mounts are opt-in. When FALSE every mount is forced read-only
    -- regardless of the per-mount flag.
    allow_readwrite  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Pre-create the singleton row so handlers can always assume id=1 exists.
INSERT INTO host_mount_policy (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS host_mounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Exactly one of conversation_id / project_id is set (the scope).
    conversation_id UUID REFERENCES conversations(id) ON DELETE CASCADE,
    project_id      UUID REFERENCES projects(id) ON DELETE CASCADE,
    -- Owner of the scope; rows are read/written scoped to this user.
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Array of { "host_path": "/abs/path", "read_only": true }.
    mounts          JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT host_mounts_one_scope
        CHECK ((conversation_id IS NULL) <> (project_id IS NULL))
);

-- One row per scope (mirrors mcp_settings' scoping).
CREATE UNIQUE INDEX IF NOT EXISTS host_mounts_conversation_uq
    ON host_mounts (conversation_id) WHERE conversation_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS host_mounts_project_uq
    ON host_mounts (project_id) WHERE project_id IS NOT NULL;

-- Grant the new permissions to the Administrators system group. Admin already
-- has the `*` wildcard so this is informational / forward-looking.
DO $$
DECLARE
    perm TEXT;
    target_rows BIGINT;
BEGIN
    SELECT count(*) INTO target_rows
    FROM groups
    WHERE name = 'Administrators'
      AND is_system = TRUE;

    IF target_rows = 0 THEN
        RAISE WARNING 'migration 10000000000005 (desktop #5): no Administrators group found; host_mount permissions will NOT be granted explicitly. Wildcard `*` on admin still applies.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'host_mount::read',
        'host_mount::manage'
    ]
    LOOP
        UPDATE groups
        SET permissions = array_append(permissions, perm),
            updated_at = NOW()
        WHERE name = 'Administrators'
          AND is_system = TRUE
          AND NOT (perm = ANY(permissions));
    END LOOP;
END $$;

COMMENT ON TABLE host_mount_policy IS
    'Singleton (id=1) deployment policy for the desktop host-folder mount feature: enabled toggle, allowed host path prefixes, read-write opt-in.';
COMMENT ON TABLE host_mounts IS
    'Per-scope (conversation XOR project) list of host folders mounted into the code sandbox. Resolved at execute_command time with read-through fallback from conversation to its project.';
