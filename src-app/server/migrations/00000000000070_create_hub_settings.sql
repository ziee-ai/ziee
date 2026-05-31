-- Deployment-wide hub catalog settings (singleton). Stores the admin's
-- pinned catalog version so the whole server serves ONE version to
-- every user.
--
-- pinned_version semantics:
--   NULL  → "track latest": refresh fetches the newest GitHub release
--           (newest stable, else newest prerelease).
--   'X.Y.Z' (or 'X.Y.Z-alpha') → pin: refresh fetches exactly that tag.
--
-- Singleton enforcement mirrors code_sandbox_settings:
-- `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)` — at most one
-- row, ever; the seed below guarantees it exists so handlers never do a
-- load-or-create dance.
CREATE TABLE hub_settings (
    id              BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE),
    -- Pinned catalog version WITHOUT the leading 'v' (e.g. '0.0.2-alpha').
    -- NULL = track latest.
    pinned_version  VARCHAR(32),
    updated_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

INSERT INTO hub_settings (id, pinned_version) VALUES (TRUE, NULL);

COMMENT ON TABLE hub_settings IS
    'Singleton deployment-wide hub catalog settings (admin-pinned version).';
COMMENT ON COLUMN hub_settings.pinned_version IS
    'Admin-pinned catalog version (semver, no leading v). NULL = track latest GitHub release.';
