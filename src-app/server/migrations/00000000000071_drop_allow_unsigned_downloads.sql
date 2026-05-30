-- Drop the allow_unsigned_downloads opt-in from runtime settings.
-- The supply-chain gate it backed is gone: engine binary downloads now
-- proceed unconditionally (cosign-keyless verify in engine/download.rs
-- remains best-effort and logs a warning when the sibling .sig is
-- missing, but no longer blocks the install). Operators who need
-- stricter handling pre-stage the binary out-of-band.

ALTER TABLE llm_runtime_settings DROP COLUMN allow_unsigned_downloads;
