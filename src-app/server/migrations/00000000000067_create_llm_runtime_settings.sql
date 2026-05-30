-- P1.b: singleton runtime settings (mirrors code_sandbox_settings).
-- One row, identified by id=TRUE; INSERT below seeds the defaults.

CREATE TABLE llm_runtime_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    -- Reaper unloads engines idle for this long. 0 = never.
    idle_unload_secs INTEGER NOT NULL DEFAULT 1800
        CHECK (idle_unload_secs >= 0 AND idle_unload_secs <= 86400),
    -- How long the auto-start coordinator waits for /health = Ok.
    auto_start_timeout_secs INTEGER NOT NULL DEFAULT 30
        CHECK (auto_start_timeout_secs BETWEEN 1 AND 600),
    -- How long the reaper waits for in-flight requests to complete
    -- before SIGTERMing the engine on idle eviction.
    drain_timeout_secs INTEGER NOT NULL DEFAULT 30
        CHECK (drain_timeout_secs BETWEEN 1 AND 600),
    -- Defense-in-depth: when false (default), cosign-keyless verify
    -- of engine binary downloads is required. Operators can opt in
    -- to TOFU downloads during the bootstrap period (before the fork
    -- repos publish signed releases).
    allow_unsigned_downloads BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

INSERT INTO llm_runtime_settings (id) VALUES (TRUE);
