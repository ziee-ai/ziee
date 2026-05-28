-- Magic-link login tokens.
--
-- Issued by the desktop user from the Remote Access settings page;
-- consumed by a phone scanning the QR (or visiting the plaintext URL
-- in the same Card). Single-use + short TTL — the plaintext token
-- itself never persists, we store SHA-256(token) so a DB dump leaks
-- nothing useful.
--
-- Issuance is admin-only AND gated by the localhost-Host middleware on
-- the Remote Access module (so a phone with a stolen admin token still
-- can't mint new magic links from outside the desktop).
-- Exchange is unauthenticated (intentional — that's the point of the
-- magic link) but heavily rate-limited (10/IP/min) per the handler.

CREATE TABLE IF NOT EXISTS magic_link_tokens (
    -- SHA-256 hex of the random plaintext token. Plaintext is returned
    -- ONCE on issue and never stored.
    token_hash    TEXT PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at    TIMESTAMPTZ NOT NULL,
    used_at       TIMESTAMPTZ NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_magic_link_tokens_expires
    ON magic_link_tokens(expires_at);

CREATE INDEX IF NOT EXISTS idx_magic_link_tokens_user
    ON magic_link_tokens(user_id, created_at DESC);

COMMENT ON TABLE magic_link_tokens IS
    'One-time login tokens issued by the desktop admin for phone/browser logins via the Remote Access tunnel. Plaintext token returned ONCE on issue and never stored (SHA-256 hash is the primary key). Single-use, 5-min TTL.';
