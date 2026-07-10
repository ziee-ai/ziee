-- Voice dictation: managed whisper.cpp speech-to-text runtime.
--
-- Mirrors the llm_local_runtime schema (migrations 20/21/66/67/68) but scoped
-- to a SINGLE hot-swappable whisper-server instance (whisper transcribes with
-- one model at a time), so the instance table is a singleton (id = TRUE) rather
-- than one row per model. Three tables:
--   * voice_runtime_versions  — installed whisper-server binaries (per platform/arch/backend)
--   * voice_runtime_instance  — the single managed whisper-server (singleton)
--   * voice_runtime_settings  — deployment-wide config (singleton)
--
-- The deploy-level kill switch lives in config (`voice: { enabled: false }`);
-- `voice_runtime_settings.enabled` is the runtime admin toggle (distinct).

-- Installed whisper-server binary versions. One default across the deployment
-- (single engine), enforced by the partial unique index.
CREATE TABLE voice_runtime_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version VARCHAR(100) NOT NULL,
    platform VARCHAR(50) NOT NULL,
    arch VARCHAR(50) NOT NULL,
    backend VARCHAR(50) NOT NULL,
    binary_path TEXT NOT NULL,
    is_system_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (version, platform, arch, backend)
);

CREATE UNIQUE INDEX voice_runtime_versions_one_default
    ON voice_runtime_versions (is_system_default)
    WHERE is_system_default = TRUE;

-- The single managed whisper-server instance (singleton row, id = TRUE).
-- Persisted so the health state survives a server restart (ensure_restored)
-- and the admin can read it via GET /api/voice/instance.
CREATE TABLE voice_runtime_instance (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    runtime_version_id UUID REFERENCES voice_runtime_versions(id) ON DELETE SET NULL,
    active_model VARCHAR(100),
    local_port INTEGER,
    base_url TEXT,
    -- Coarse lifecycle: stopped | running.
    status VARCHAR(20) NOT NULL DEFAULT 'stopped'
        CHECK (status IN ('stopped', 'running')),
    -- Fine health-state-machine name (mirrors llm_runtime_instances.state).
    state VARCHAR(30) NOT NULL DEFAULT 'stopped',
    state_changed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    restart_attempts INTEGER NOT NULL DEFAULT 0,
    last_failure_reason TEXT,
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

INSERT INTO voice_runtime_instance (id) VALUES (TRUE) ON CONFLICT (id) DO NOTHING;

-- Deployment-wide voice settings (singleton). Mirrors llm_runtime_settings
-- (migration 67) WITHOUT allow_unsigned_downloads (dropped upstream in
-- migration 71), plus the dictation-specific model/language/caps.
CREATE TABLE voice_runtime_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    -- Runtime admin toggle (distinct from the config deploy kill switch).
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    -- Selected whisper ggml model name (tiny | base | base.en | small).
    model VARCHAR(50) NOT NULL DEFAULT 'base',
    -- Default transcription language ('auto' = whisper auto-detect).
    language VARCHAR(20) NOT NULL DEFAULT 'auto',
    -- Reaper unloads the whisper-server after this idle period. 0 = never.
    idle_unload_secs INTEGER NOT NULL DEFAULT 1800
        CHECK (idle_unload_secs >= 0 AND idle_unload_secs <= 86400),
    -- How long auto-start waits for whisper-server /health (whisper model load
    -- can be slow on cold start, so the default is higher than the LLM runtime).
    auto_start_timeout_secs INTEGER NOT NULL DEFAULT 60
        CHECK (auto_start_timeout_secs BETWEEN 1 AND 600),
    -- How long the reaper waits for in-flight transcriptions before SIGTERM.
    drain_timeout_secs INTEGER NOT NULL DEFAULT 30
        CHECK (drain_timeout_secs BETWEEN 1 AND 600),
    -- Server-side clip length cap (seconds).
    max_clip_seconds INTEGER NOT NULL DEFAULT 120
        CHECK (max_clip_seconds BETWEEN 1 AND 3600),
    -- Server-side upload size cap (bytes); default 32 MiB. Ceiling is 64 MiB to
    -- match the per-route DefaultBodyLimit (VOICE_TRANSCRIBE_BODY_LIMIT) — a
    -- higher setting would be rejected with a 413 before the handler's logical
    -- cap ever ran. 64 MiB is ample (a 120s 16kHz mono WAV is ~3.8 MB).
    max_upload_bytes BIGINT NOT NULL DEFAULT 33554432
        CHECK (max_upload_bytes BETWEEN 1024 AND 67108864),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

INSERT INTO voice_runtime_settings (id) VALUES (TRUE) ON CONFLICT (id) DO NOTHING;
