-- Remote Access singleton settings + permission grants.
--
-- Owns the ngrok tunnel lifecycle config: the auth token (encrypted at
-- rest, mirroring 06-llm-provider F-02 mitigation), the optional
-- reserved/custom domain (paid plans only; blank = ngrok auto-assigns),
-- auto-start-on-boot toggle (only meaningful with a fixed domain —
-- enforced by a CHECK constraint), and the password-auth-enabled
-- toggle that gates whether the tunneled login page renders a password
-- form at all.
--
-- Singleton row (id=1) following the memory_admin_settings pattern
-- from migration 56.

CREATE TABLE IF NOT EXISTS remote_access_settings (
    id                    SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    -- ngrok auth token, encrypted with the deployment's storage_key
    -- (see core::secrets::storage_key + common::secret::encrypt_secret).
    -- Stored bytea so the plaintext never lands on disk.
    ngrok_auth_token_enc  BYTEA NULL,
    -- Optional reserved/custom domain (e.g. "my-app.ngrok.app" for
    -- ngrok-reserved subdomains, or a full custom domain for paid
    -- plans). NULL = ngrok assigns an ephemeral *.ngrok-free.app URL
    -- on every connect.
    ngrok_domain          TEXT NULL,
    -- Auto-start the tunnel on server boot. Only meaningful WITH a
    -- fixed domain — an auto-assigned ephemeral URL would change
    -- across restarts and break any link the user had previously
    -- shared. The CHECK constraint enforces the invariant; the API
    -- layer auto-flips this to FALSE when the domain is cleared.
    auto_start_tunnel     BOOLEAN NOT NULL DEFAULT FALSE,
    -- Whether the tunneled login page renders a password form. OFF by
    -- default: magic-link QR is the only first-login path. Admin
    -- explicitly opts in to a password fallback, and only after they
    -- rotate the bootstrap admin password (enforced in the API layer
    -- against users.password_changed_at).
    password_auth_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT remote_access_auto_start_requires_domain
        CHECK (auto_start_tunnel = FALSE OR ngrok_domain IS NOT NULL)
);

-- Pre-create the singleton row so handlers can always assume id=1
-- exists. Avoids race conditions on first read.
INSERT INTO remote_access_settings (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;

-- Grant the new permissions to the Administrators system group. Admin
-- already has the `*` wildcard so this is informational / forward-
-- looking; explicit grant lets us drop the wildcard later without
-- breaking remote-access.
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
        RAISE WARNING 'migration 10000000000003 (desktop #3): no Administrators group found; remote_access permissions will NOT be granted explicitly. Wildcard `*` on admin still applies.';
    END IF;

    FOREACH perm IN ARRAY ARRAY[
        'remote_access::read',
        'remote_access::manage'
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

COMMENT ON TABLE remote_access_settings IS
    'Singleton config row (id=1) for the Remote Access feature: ngrok auth token + optional reserved domain + auto-start gate + password-auth opt-in.';
