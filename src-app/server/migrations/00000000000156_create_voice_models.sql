-- Voice whisper-MODEL management (download / upload / library).
--
-- Adds a per-installed-model table (mirrors voice_runtime_versions for binaries),
-- an admin-configurable model source repo, and a value CHECK on the instance
-- health-state column (parity with llm_runtime_instances, migration 66).

-- Installed whisper ggml/gguf model files. A model has a stable identity (its
-- filename under <app_data>/voice-models/), a provenance (`source`), an integrity
-- flag (`verified` = bytes matched the HF-advertised git-LFS oid / a pinned
-- digest), and the recorded sha256 for update-detection.
CREATE TABLE voice_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Short model name (also the settings.model pointer value); <= 50 to fit
    -- voice_runtime_settings.model.
    name VARCHAR(50) NOT NULL,
    -- On-disk filename under voice-models/ (e.g. ggml-base.en.bin). Unique: one
    -- row per installed file.
    filename VARCHAR(200) NOT NULL UNIQUE,
    -- How this model was acquired.
    source VARCHAR(20) NOT NULL DEFAULT 'catalog'
        CHECK (source IN ('catalog', 'url', 'upload')),
    -- The URL/repo it came from (NULL for uploads).
    source_url TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    -- Lowercase hex sha256 of the file (computed at install; used for
    -- update-detection). NULL only transiently before a size/hash is known.
    sha256 CHAR(64),
    -- TRUE when the bytes matched a source-of-truth digest (catalog/HF oid);
    -- FALSE for arbitrary-URL downloads and uploads (no pin to check against).
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX voice_models_one_name ON voice_models (name);

-- Admin-configurable model source repo (default upstream). If upstream
-- renames/moves, or an operator wants an internal HF mirror, they repoint this
-- with no code change. Read by the runtime catalog fetch.
ALTER TABLE voice_runtime_settings
    ADD COLUMN model_source_repo VARCHAR(200) NOT NULL DEFAULT 'ggerganov/whisper.cpp';

-- Value CHECK on the health-state column (parity with llm_runtime_instances,
-- migration 66). The singleton's existing state is always one of these names
-- (written only by the health state machine), so no pre-existing row violates it.
ALTER TABLE voice_runtime_instance
    ADD CONSTRAINT voice_runtime_instance_state_check
    CHECK (state IN ('starting', 'healthy', 'unhealthy',
                     'crashed', 'restarting', 'failed', 'stopped'));
