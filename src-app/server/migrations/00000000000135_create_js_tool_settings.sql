-- Singleton row of runtime-configurable limits for the built-in `run_js`
-- programmatic-tool-calling tool (js_tool). Lets admins tune the embedded-
-- interpreter + orchestration caps from the UI without rebuilding the server.
-- Mirrors code_sandbox_settings (migration 41).
--
-- The hardcoded values these replace at runtime:
--   - js_tool::runtime::JsLimits::default (memory_bytes, max_stack_bytes)
--   - js_tool::limits::JsCaps::default (wall, approval_timeout)
--   - js_tool::executor consts (MAX_CONCURRENT_RUNS, MAX_CONCURRENT_DISPATCH,
--     MAX_TRACE_ENTRIES)
--
-- Singleton enforcement: `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)`
-- — at most one row, ever; the seed below guarantees it exists.

CREATE TABLE js_tool_settings (
    id                        BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),

    -- In-interpreter caps (rquickjs set_memory_limit / set_max_stack_size).
    memory_bytes              BIGINT  NOT NULL DEFAULT 134217728,  -- 128 MiB
    max_stack_bytes           BIGINT  NOT NULL DEFAULT 524288,     -- 512 KiB

    -- Orchestration caps.
    wall_secs                 INTEGER NOT NULL DEFAULT 300,        -- active-execution wall-clock
    approval_timeout_secs     INTEGER NOT NULL DEFAULT 300,        -- per-approval wait
    max_concurrent_runs       INTEGER NOT NULL DEFAULT 8,          -- process-global interpreter admission
    max_concurrent_dispatch   INTEGER NOT NULL DEFAULT 6,          -- per-run parallel sub-tool dispatch
    max_trace_entries         INTEGER NOT NULL DEFAULT 256,        -- per-run recorded sub-call trace cap

    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Defense-in-depth value-range guards. The handler validates again before
    -- writing (clearer 422) but the DB is the last line. Upper bounds prevent an
    -- admin from OOMing the server (runs × memory) or hanging a runtime for hours.
    CONSTRAINT memory_bytes_range            CHECK (memory_bytes            BETWEEN 16777216 AND 4294967296),  -- 16 MiB .. 4 GiB
    CONSTRAINT max_stack_bytes_range         CHECK (max_stack_bytes         BETWEEN 65536 AND 67108864),       -- 64 KiB .. 64 MiB
    CONSTRAINT wall_secs_range               CHECK (wall_secs               BETWEEN 1 AND 3600),
    CONSTRAINT approval_timeout_secs_range   CHECK (approval_timeout_secs   BETWEEN 5 AND 3600),
    CONSTRAINT max_concurrent_runs_range     CHECK (max_concurrent_runs     BETWEEN 1 AND 256),
    CONSTRAINT max_concurrent_dispatch_range CHECK (max_concurrent_dispatch BETWEEN 1 AND 64),
    CONSTRAINT max_trace_entries_range       CHECK (max_trace_entries       BETWEEN 1 AND 10000)
);

COMMENT ON TABLE js_tool_settings IS
    'Singleton row of runtime-tunable run_js (js_tool) limits. Mirrors code_sandbox_settings.';

-- Seed the singleton with defaults. Idempotent on migration rerun.
INSERT INTO js_tool_settings (id) VALUES (TRUE)
ON CONFLICT (id) DO NOTHING;

-- Admin-only: `js_tool::settings::{read,manage}` are held by the Administrators
-- group's `*` wildcard (see migration 1). No separate grant migration. We
-- deliberately do NOT grant them to the Users group — regular users may USE
-- run_js (js_tool::use, migration 134) but must not see or tune its caps.
