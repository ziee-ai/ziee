-- Singleton row of runtime-configurable resource limits for the code
-- sandbox (Plan 1 §6). Lets admins tune memory/CPU/PID/timeout caps from
-- the UI without rebuilding the server.
--
-- The hardcoded values these replace at runtime:
--   - sandbox::build_bwrap_argv (`prlimit --as / --fsize / --nproc / --nofile / --cpu`)
--   - cgroup::CgroupScope::create (memory.max / pids.max / cpu.max)
--   - DEFAULT_TIMEOUT_SECS (wall-clock per-exec budget)
--   - mac_vm / wsl2 VM_IDLE_EVICT_SECS (VM idle eviction, 0 = never)
--
-- Singleton enforcement: `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK
-- (id = TRUE)` — the table can hold at most one row, ever, and that row
-- is implicitly named `id = TRUE`. No need for application-side
-- "load-or-create" dances; the seed below guarantees the row exists.

CREATE TABLE code_sandbox_settings (
    id                     BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- cgroup v2 (defended by GuestCgroup on VM backends, by CgroupScope on Linux).
    memory_max_bytes       BIGINT  NOT NULL DEFAULT 536870912,  -- 512 MiB
    memory_swap_max_bytes  BIGINT  NOT NULL DEFAULT 0,          -- no swap
    pids_max               INTEGER NOT NULL DEFAULT 256,
    -- "<quota> <period>" in microseconds. "100000 100000" = 1 CPU.
    cpu_max                TEXT    NOT NULL DEFAULT '100000 100000',

    -- prlimit backstops (always-on; the cgroup is defense-in-depth).
    address_space_bytes    BIGINT  NOT NULL DEFAULT 4294967296, -- 4 GiB (--as)
    fsize_bytes            BIGINT  NOT NULL DEFAULT 268435456,  -- 256 MiB (--fsize)
    nproc_max              INTEGER NOT NULL DEFAULT 256,        -- --nproc
    nofile_max             INTEGER NOT NULL DEFAULT 1024,       -- --nofile
    -- CPU-seconds (--cpu). Generous so it never preempts a legitimate
    -- long-running command before the wall-clock SIGKILL does.
    cpu_secs_max           INTEGER NOT NULL DEFAULT 1240,

    -- Wall-clock budget for one `execute_command`. Matches the
    -- DEFAULT_TIMEOUT_SECS = 620 the agent / vm_client enforce.
    timeout_secs           INTEGER NOT NULL DEFAULT 620,

    -- mac_vm / wsl2 VM idle eviction. 0 = never evict.
    vm_idle_evict_secs     INTEGER NOT NULL DEFAULT 900,

    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Defense-in-depth value-range guards. The handler validates again
    -- before writing (clearer error messages) but the DB is the last line.
    CONSTRAINT memory_max_bytes_positive       CHECK (memory_max_bytes      >= 16777216),    -- ≥ 16 MiB
    CONSTRAINT memory_swap_max_bytes_nonneg    CHECK (memory_swap_max_bytes >= 0),
    CONSTRAINT pids_max_positive               CHECK (pids_max              BETWEEN 8 AND 100000),
    CONSTRAINT address_space_bytes_positive    CHECK (address_space_bytes   >= 16777216),
    CONSTRAINT fsize_bytes_positive            CHECK (fsize_bytes           >= 1048576),     -- ≥ 1 MiB
    CONSTRAINT nproc_max_positive              CHECK (nproc_max             BETWEEN 8 AND 100000),
    CONSTRAINT nofile_max_positive             CHECK (nofile_max            BETWEEN 64 AND 1048576),
    CONSTRAINT cpu_secs_max_positive           CHECK (cpu_secs_max          BETWEEN 10 AND 86400),
    CONSTRAINT timeout_secs_positive           CHECK (timeout_secs          BETWEEN 5 AND 86400),
    CONSTRAINT vm_idle_evict_secs_nonneg       CHECK (vm_idle_evict_secs    >= 0),
    -- Cheap shape-check on cpu_max ("<digits> <digits>"). The Rust handler
    -- does a stricter parse before write; this is just the "obviously bogus"
    -- gate at the DB level.
    CONSTRAINT cpu_max_shape                   CHECK (cpu_max ~ '^[0-9]+ [0-9]+$')
);

COMMENT ON TABLE code_sandbox_settings IS
    'Singleton row of runtime-tunable code_sandbox resource limits. Plan 1 §6.';

-- Seed the singleton with defaults. Idempotent on migration rerun.
INSERT INTO code_sandbox_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;

-- The `code_sandbox::resource_limits::manage` permission is implicitly held
-- by the Administrators group's `*` wildcard (see migration 1's seed of the
-- Administrators system group). No separate grant migration is needed.
-- Read access is the same: admins have it via `*`. We deliberately do NOT
-- grant either to the Users group: regular users can SEE that a sandbox
-- exists but should not see or tune its resource caps.
