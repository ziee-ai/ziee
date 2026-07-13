-- ziee deploy-time seed — DEPLOY-ONLY (lives only on the `deploy` branch).
--
-- Replaces the removed `desired_state` boot reconciler (PR #145) with plain SQL
-- applied ONCE PER DEPLOY, AFTER the app has run its migrations. It is run by the
-- `ziee-seed` one-shot in docker-compose.deploy.yml, which gates on
-- `ziee-web: service_healthy` (the app migrates before it reports healthy, so all
-- tables/columns below already exist).
--
-- WHAT IT DOES (all idempotent — safe to re-run every deploy):
--   1. Registers the 3 org system MCP servers (rcpa/dscc/biognosia), enabled, in
--      the default Users group.
--   2. Fills + enables the pre-seeded `google` auth provider (app-compatible
--      pgcrypto encryption of the client secret).
--   3. Reduces the default Users group's permissions (hides feature surfaces).
--   4. Creates the first admin (create-only; never reverts a later UI change).
--
-- SECRETS: this file contains ONLY psql `:placeholders` — NEVER a secret literal.
-- The `ziee-seed` service passes real values via `psql -v` from the deploy env.
-- `\set ON_ERROR_STOP on` + the guards below make a missing/empty required value
-- abort LOUD, so we never silently seed a blank or plaintext secret.
--
-- pgcrypto (crypt/gen_salt/pgp_sym_encrypt) is created by migration 43, so it is
-- already present — this file does NOT `CREATE EXTENSION` (would need superuser).

\set ON_ERROR_STOP on

-- ── Fail-loud: every required value must be present + non-empty ───────────────
-- psql does NOT interpolate `:'var'` inside a dollar-quoted block, so the checks
-- are computed here in plain SQL (interpolation works), captured with \gset, then
-- asserted with \if + a static RAISE. With ON_ERROR_STOP the RAISE exits psql
-- non-zero, so a missing/empty required value aborts LOUD before any write — we
-- never silently seed a blank or plaintext secret. (Runs before BEGIN, so a failed
-- guard leaves no open transaction.)
SELECT
    CASE WHEN :'storage_key'    = '' THEN 1 ELSE 0 END AS g_storage_key_empty,
    CASE WHEN length(:'storage_key') < 32 THEN 1 ELSE 0 END AS g_storage_key_short,
    CASE WHEN :'client_id'      = '' THEN 1 ELSE 0 END AS g_client_id_empty,
    CASE WHEN :'client_secret'  = '' THEN 1 ELSE 0 END AS g_client_secret_empty,
    CASE WHEN :'admin_username' = '' THEN 1 ELSE 0 END AS g_admin_username_empty,
    CASE WHEN :'admin_email'    = '' THEN 1 ELSE 0 END AS g_admin_email_empty,
    CASE WHEN :'admin_password' = '' THEN 1 ELSE 0 END AS g_admin_password_empty,
    CASE WHEN :'rcpa_url'       = '' THEN 1 ELSE 0 END AS g_rcpa_url_empty,
    CASE WHEN :'dscc_url'       = '' THEN 1 ELSE 0 END AS g_dscc_url_empty,
    CASE WHEN :'biognosia_url'  = '' THEN 1 ELSE 0 END AS g_biognosia_url_empty
\gset

\if :g_storage_key_empty
\echo '>>> seed ABORT: ZIEE_STORAGE_KEY (storage_key) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: ZIEE_STORAGE_KEY (storage_key) is empty — refusing to seed'; END $$;
\endif
\if :g_storage_key_short
\echo '>>> seed ABORT: ZIEE_STORAGE_KEY shorter than 32 chars'
DO $$ BEGIN RAISE EXCEPTION 'seed: ZIEE_STORAGE_KEY must be >= 32 chars (matches the app crypto guard)'; END $$;
\endif
\if :g_client_id_empty
\echo '>>> seed ABORT: GOOGLE_CLIENT_ID (client_id) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: GOOGLE_CLIENT_ID (client_id) is empty'; END $$;
\endif
\if :g_client_secret_empty
\echo '>>> seed ABORT: GOOGLE_CLIENT_SECRET (client_secret) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: GOOGLE_CLIENT_SECRET (client_secret) is empty'; END $$;
\endif
\if :g_admin_username_empty
\echo '>>> seed ABORT: ZIEE_ADMIN_USERNAME (admin_username) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: ZIEE_ADMIN_USERNAME (admin_username) is empty'; END $$;
\endif
\if :g_admin_email_empty
\echo '>>> seed ABORT: ZIEE_ADMIN_EMAIL (admin_email) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: ZIEE_ADMIN_EMAIL (admin_email) is empty'; END $$;
\endif
\if :g_admin_password_empty
\echo '>>> seed ABORT: ZIEE_ADMIN_PASSWORD (admin_password) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: ZIEE_ADMIN_PASSWORD (admin_password) is empty'; END $$;
\endif
\if :g_rcpa_url_empty
\echo '>>> seed ABORT: RCPA_MCP_URL (rcpa_url) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: RCPA_MCP_URL (rcpa_url) is empty'; END $$;
\endif
\if :g_dscc_url_empty
\echo '>>> seed ABORT: DSCC_MCP_URL (dscc_url) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: DSCC_MCP_URL (dscc_url) is empty'; END $$;
\endif
\if :g_biognosia_url_empty
\echo '>>> seed ABORT: BIOGNOSIA_MCP_URL (biognosia_url) is empty'
DO $$ BEGIN RAISE EXCEPTION 'seed: BIOGNOSIA_MCP_URL (biognosia_url) is empty'; END $$;
\endif

BEGIN;

-- ── 1. System MCP servers — idempotent BY NAME ───────────────────────────────
-- `mcp_servers.name` has NO unique constraint, and the retired desired_state
-- reconciler already created these rows under deterministic v5 UUIDs on any DB it
-- touched. So we key on (name, is_system): create only when absent, then re-assert
-- ("enforce") the declared fields on the existing row. This is correct on a fresh
-- DB and on one the old reconciler already seeded — never a duplicate row.
--
-- All 3 are is_system (no owner), is_built_in=false (real external HTTP endpoint),
-- transport_type='http', usage_mode='auto' (model decides when to call the tools).
-- `enabled=true` is re-asserted every deploy on purpose: ziee's boot health check
-- auto-disables an unreachable server, and this flips it back on when the endpoint
-- is up again.

-- rcpa
INSERT INTO mcp_servers (id, user_id, name, display_name, description,
        enabled, is_system, is_built_in, transport_type, url, usage_mode,
        supports_sampling, timeout_seconds)
SELECT gen_random_uuid(), NULL, 'rcpa', 'RCPA', 'RCPA analysis tools',
        true, true, false, 'http', :'rcpa_url', 'auto', false, 300
WHERE NOT EXISTS (SELECT 1 FROM mcp_servers WHERE name = 'rcpa' AND is_system);

UPDATE mcp_servers
   SET url = :'rcpa_url', enabled = true, usage_mode = 'auto',
       display_name = 'RCPA', description = 'RCPA analysis tools',
       supports_sampling = false, transport_type = 'http', timeout_seconds = 300,
       updated_at = NOW()
 WHERE name = 'rcpa' AND is_system;

-- dscc
INSERT INTO mcp_servers (id, user_id, name, display_name, description,
        enabled, is_system, is_built_in, transport_type, url, usage_mode,
        supports_sampling, timeout_seconds)
SELECT gen_random_uuid(), NULL, 'dscc', 'DSCC', 'DSCC analysis tools',
        true, true, false, 'http', :'dscc_url', 'auto', false, 300
WHERE NOT EXISTS (SELECT 1 FROM mcp_servers WHERE name = 'dscc' AND is_system);

UPDATE mcp_servers
   SET url = :'dscc_url', enabled = true, usage_mode = 'auto',
       display_name = 'DSCC', description = 'DSCC analysis tools',
       supports_sampling = false, transport_type = 'http', timeout_seconds = 300,
       updated_at = NOW()
 WHERE name = 'dscc' AND is_system;

-- biognosia (may issue MCP sampling/createMessage back to the app)
INSERT INTO mcp_servers (id, user_id, name, display_name, description,
        enabled, is_system, is_built_in, transport_type, url, usage_mode,
        supports_sampling, timeout_seconds)
SELECT gen_random_uuid(), NULL, 'biognosia', 'Biognosia', 'Biognosia tools',
        true, true, false, 'http', :'biognosia_url', 'auto', true, 300
WHERE NOT EXISTS (SELECT 1 FROM mcp_servers WHERE name = 'biognosia' AND is_system);

UPDATE mcp_servers
   SET url = :'biognosia_url', enabled = true, usage_mode = 'auto',
       display_name = 'Biognosia', description = 'Biognosia tools',
       supports_sampling = true, transport_type = 'http', timeout_seconds = 300,
       updated_at = NOW()
 WHERE name = 'biognosia' AND is_system;

-- ── 2. Assign the 3 servers to the default Users group (lookup by name) ───────
-- A system server in no group is unusable by non-admin users. Additive + idempotent.
INSERT INTO user_group_mcp_servers (group_id, mcp_server_id)
SELECT g.id, s.id
  FROM groups g, mcp_servers s
 WHERE g.name = 'Users' AND s.is_system AND s.name IN ('rcpa', 'dscc', 'biognosia')
ON CONFLICT (group_id, mcp_server_id) DO NOTHING;

-- ── 3. Google auth provider — fill + enable (app-compatible crypto) ───────────
-- The `google` row is pre-seeded (disabled) by migration 47. We stamp client_id
-- into config, encrypt client_secret into the authoritative `client_secret_encrypted`
-- column exactly as the app does (`pgp_sym_encrypt(secret, ZIEE_STORAGE_KEY)`, see
-- common/secret.rs), blank the legacy plaintext copy in config, and enable it.
UPDATE auth_providers
   SET config = jsonb_set(
                    jsonb_set(config, '{client_id}', to_jsonb(:'client_id'::text)),
                    '{client_secret}', '""'::jsonb),
       client_secret_encrypted = pgp_sym_encrypt(:'client_secret', :'storage_key'),
       enabled = true,
       updated_at = NOW()
 WHERE name = 'google';

-- ── 4. Reduce the default Users group's permissions ──────────────────────────
-- Hide feature surfaces from regular users (the permission gates nav + settings tab
-- + route together). Idempotent prefix filter; re-running yields the identical set,
-- so a future migration that re-grants one of these to Users is self-correcting.
-- KEEPS `notifications::read` (distinct prefix). Administrators untouched.
UPDATE groups
   SET permissions = ARRAY(
           SELECT p
             FROM unnest(permissions) WITH ORDINALITY AS t(p, ord)
            WHERE p NOT LIKE 'projects::%'
              AND p NOT LIKE 'hub::%'
              AND p NOT LIKE 'knowledge_base::%'
              AND p NOT LIKE 'scheduler::%'
              AND p NOT LIKE 'assistants::%'
              AND p NOT LIKE 'web_search::%'
              AND p NOT LIKE 'lit_search::%'
              AND p NOT LIKE 'workflows::%'
              AND p NOT LIKE 'memory::%'
              AND p NOT LIKE 'citations::%'
            ORDER BY ord),
       updated_at = NOW()
 WHERE name = 'Users';

-- ── 5. First admin — create-only ─────────────────────────────────────────────
-- bcrypt via pgcrypto (cost 12 = the app's DEFAULT_COST → $2a$12$…, verify-compatible
-- with bcrypt::verify). Bare ON CONFLICT DO NOTHING covers the username/email UNIQUE
-- constraints AND the `unique_root_admin` partial index (only one is_admin=true row),
-- so on any DB that already has an admin this is a clean no-op — a UI-changed admin
-- password is never reverted by a redeploy.
INSERT INTO users (username, email, password_hash, is_admin, is_active, email_verified)
VALUES (:'admin_username', :'admin_email',
        crypt(:'admin_password', gen_salt('bf', 12)), true, true, true)
ON CONFLICT DO NOTHING;

-- Assign the admin to the Administrators group (no-op if the insert above was a
-- no-op because a different root admin already exists — the username won't match).
INSERT INTO user_groups (user_id, group_id)
SELECT u.id, g.id
  FROM users u, groups g
 WHERE u.username = :'admin_username'
   AND g.name = 'Administrators' AND g.is_system
ON CONFLICT DO NOTHING;

COMMIT;

\echo 'ziee seed applied successfully.'
