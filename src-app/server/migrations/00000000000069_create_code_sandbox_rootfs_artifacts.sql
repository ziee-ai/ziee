-- code_sandbox rootfs version manager (Plan 5)
--
-- Aligns the sandbox-rootfs lifecycle with the local-llm-runtime
-- pattern: server discovers versions live via the GitHub Releases API,
-- admin downloads + pins explicitly, drain-then-swap semantics replace
-- the old static `known_revisions.toml` resolver.
--
-- Two surfaces:
--
-- 1. `code_sandbox_settings.current_rootfs_version` — one semver pin
--    shared across all flavors + arches. NULL on a fresh install; set
--    by `version_manager::ensure_pin_initialized` to the latest
--    GitHub release the first time the server can reach the API.
--    Admin can change it later from the settings UI; the change-pin
--    handler runs the drain + (on major bump) install-cache wipe.
--
-- 2. `code_sandbox_rootfs_artifacts` — what's actually downloaded to
--    disk. Independent of the pin: an admin can pre-download a v0.2.0
--    artifact while still pinned at v0.1.0, then flip the pin
--    atomically without a download wait.

ALTER TABLE code_sandbox_settings
    ADD COLUMN current_rootfs_version TEXT;

CREATE TABLE code_sandbox_rootfs_artifacts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version         TEXT NOT NULL,          -- semver, e.g. "0.1.0"
    arch            TEXT NOT NULL,          -- "x86_64" | "aarch64"
    flavor          TEXT NOT NULL,          -- "minimal" | "full"
    package         TEXT NOT NULL,          -- "squashfs" | "tar.zst"
    sha256          TEXT NOT NULL,          -- 64-hex
    artifact_path   TEXT NOT NULL,          -- absolute path in cache dir
    cosign_bundle   TEXT,                   -- absolute path to .cosign.bundle, NULL if unsigned
    status          TEXT NOT NULL DEFAULT 'installed',   -- installed | downloading | failed
    downloaded_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at    TIMESTAMPTZ,
    UNIQUE (version, arch, flavor, package)
);

CREATE INDEX idx_code_sandbox_rootfs_artifacts_version
    ON code_sandbox_rootfs_artifacts (version);

CREATE INDEX idx_code_sandbox_rootfs_artifacts_arch_flavor
    ON code_sandbox_rootfs_artifacts (arch, flavor);
