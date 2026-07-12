# FIX_ROUND-3 — third blind round

A fresh blind agent swept all 10 angles over the twice-fixed diff, with instructions not to
manufacture nits. It independently RE-VERIFIED the load-bearing fixes: the `assign_to_group` rename
is a true no-op across all five call sites; the `cfg!(debug_assertions)` gate on the new health-check
seam genuinely compiles out of release (no `debug-assertions` override in `src-app/Cargo.toml`);
`PoolConnection::close()` exists and the lock block borrow-checks; migration 157 matches migrations
7 + 25; and the `assistants::*` trim does not break chat (the picker store self-gates on
`hasPermissionNow(AssistantsRead)`, so it never 403s).

Three LOW findings, all fixed:

1. `mcp-admin-servers.spec.ts` — my `beforeEach` fixture got copy-pasted into a SECOND describe block
   whose tests never use it (dead setup + a comment describing paths that block doesn't exercise).
   Removed.
2. `desired-state-restricted-user.spec.ts` — the fixture used `p LIKE pre || '::%'`, and LIKE treats
   `_` as a single-char wildcard. Harmless for today's patterns, but a future prefix like
   `mcp_servers::*` would strip MORE than the reconciler does, making the spec assert a state
   production never reaches. Switched to `starts_with`, which mirrors `permission_matches` exactly.
3. The `add:`-rejection log blamed "a wildcard" even when the offending entry was simply blank.
   The reason is now computed from the entry.

**New confirmed findings:** 3
