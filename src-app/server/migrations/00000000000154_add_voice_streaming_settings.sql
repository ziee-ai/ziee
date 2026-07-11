-- Streaming (live-caption) voice dictation settings.
--
-- Additive columns on the voice_runtime_settings singleton (migration 151). Live
-- captions re-decode the whole accumulating clip each tick against the SAME
-- whisper-server instance/model as batch dictation, so the only new tunables are
-- an availability toggle + the interim decode cadence. No window column — the
-- full-stitched approach re-decodes the entire buffer (there is no tail window).

ALTER TABLE voice_runtime_settings
    -- Deployment-wide availability of live streaming captions (distinct from the
    -- `enabled` master toggle; live mode also requires `enabled = TRUE`).
    ADD COLUMN streaming_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    -- Interim decode cadence (ms) — the client re-decodes the accumulating buffer
    -- at most this often while recording. Lengthen on slow hardware / heavy models
    -- so interim decodes don't fall behind. Bounds match validate_settings_patch.
    ADD COLUMN stream_interval_ms INTEGER NOT NULL DEFAULT 1000
        CHECK (stream_interval_ms BETWEEN 300 AND 10000),
    -- Cost bound for the interim decode: the server clamps each interim clip to
    -- its trailing N seconds before decoding, so per-tick whisper cost is bounded
    -- regardless of recording length (protects the shared engine under concurrent
    -- live-caption users). Clips at/under this length are fully stitched; longer
    -- ones show the recent window while recording. The FINAL on-stop decode is
    -- always the full, unclamped clip.
    ADD COLUMN stream_max_decode_secs INTEGER NOT NULL DEFAULT 30
        CHECK (stream_max_decode_secs BETWEEN 5 AND 600);
