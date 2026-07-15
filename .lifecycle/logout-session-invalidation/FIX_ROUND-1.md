# FIX_ROUND-1 — findings from the blind multi-angle audit

Two fresh blind auditors (diff-only context, no access to my reasoning) across
security / perms-authz / correctness / concurrency / regressions / error-handling /
api-contract / state-management / patterns-conformance / tests-quality / forgery / test-reality.
They INDEPENDENTLY converged on the same top issues, which is why I treated them as real rather
than as speculative.

## Fixed

- **HIGH — a successor refresh token escapes a racing logout** (`refresh_tokens.rs:233`).
  `end_session_atomically` revokes with `UPDATE refresh_tokens WHERE user_id = $1 AND revoked_at IS
  NULL`. Under READ COMMITTED that UPDATE only scans rows committed at command start, so a successor
  INSERTed-but-not-COMMITTed by a concurrent `claim_rotation_and_register` is **never scanned** —
  EvalPlanQual only re-checks rows the scan already found. The successor therefore survived while the
  epoch still moved to N+1, and replaying it minted an access token stamped with the **new** epoch:
  a fully live session. **The feature defeated at precisely the point my doc-comment claimed
  "Postgres serializes this for you."** Fixed by taking the `users` row lock (`SELECT token_version
  … FOR SHARE`) as the FIRST statement of the claim transaction, forcing a serial order with logout.
  Lock order is deliberately `users → refresh_tokens` in BOTH paths, so no deadlock is introduced
  (the auditor flagged the inversion risk explicitly). Pinned by **TEST-23**.
- **MEDIUM — an already-open SSE stream survived logout** (`sync/handlers.rs:129`,
  `chat/stream/handler.rs:100`). The subscribe gate checks the epoch once; the stream then lives to
  the token's `exp` (24h default) with a 60s re-check of only `is_active` + `profile::read` —
  neither of which logout changes. The chat stream carries **live assistant content**, not
  notify-only envelopes. The client-side `Session` fan-out is not a boundary here: a holder of a
  stolen token does not run our JS. Fixed by re-checking the epoch in both re-check loops via
  `get_by_id_with_token_version` — **zero extra round-trips**, since both loops already loaded the
  user row. My `verify_token_version` INVARIANT doc had dismissed these two callers as "read `exp`
  only"; that was true at connect and wrong for the stream's lifetime. Doc corrected.
- **MEDIUM — a transient DB error during `/auth/refresh` logged users out** (`handlers.rs:557`).
  I mapped every `current_token_version` error to 401, but the client treats a 401 from
  `/auth/refresh` as terminal → `tearDownSession()` → wipe **+ `window.location.reload()`**. A pool
  blip would therefore bounce every active tab mid-work — a strictly larger blast radius than
  pre-diff, and inconsistent with the `get_by_id` mapping 30 lines above. `current_token_version`
  now returns `Option` so callers separate "user gone" (401) from "DB failed" (500).
- **MEDIUM — a HOLLOW test** (`e2e/auth/logout.spec.ts:61`). The spec billed as proving the
  server-side backstop used a **same-context** tab, whose login form appears purely via
  shared-localStorage rehydration — it never sent the revoked token anywhere and **passed with the
  entire server-side feature reverted**. Rewritten to use an independent browser context with its own
  login and its own stored token, asserting the token is **still in B's storage** and that the
  **server** returns 401 for it. Now it can only pass because of the revocation.
- **LOW — TEST-9 claimed more than it asserted.** Its name/doc sold it as proof of read-before-claim,
  but it is sequential, so the read position cannot affect the result. Renamed to
  `test_refresh_then_logout_kills_the_refreshed_session` with an explicit HONEST SCOPE note; the real
  interleaving is now covered by TEST-23.
- **LOW — `setAuthFromAutoLogin` cleared `hasPassword`** (`Auth.store.ts:623`). Desktop's mid-session
  `auto_login` re-mint reaches it for the SAME identity with no follow-up `initAuth()`, so the profile
  page would flip to "set a password". `hasPassword` is not a grant and not a leak vector — only
  `permissions` is cleared now.
- **LOW — desktop boot race** (`Auth.store.ts:223`). `tearDownSession`'s desktop safety rested solely
  on callers checking `refreshFallback`, which `desktop-base` registers **asynchronously**; a terminal
  401 during boot could slip past and reload the Tauri webview mid-startup. Added an independent
  `'__TAURI__' in window` guard inside `tearDownSession` itself. Probes the RUNTIME, not the build, so
  the ngrok/phone surface (same bundle, outside Tauri) still correctly reloads to `PhoneAuthPage`.
- **LOW — overclaiming doc.** The logout doc said "Ends EVERY session"; the legacy jti-less refresh
  branch has no `refresh_tokens` row to revoke and re-mints with the current epoch. Pre-existing
  (`revoke_all_for_user` never covered it either) and effectively extinct. Claim softened to state the
  residue explicitly rather than closing the branch (its own change).

## Accepted as out of scope (recorded, not fixed — causality rule)

- **Download tokens** (`file/types.rs:79`) — a separate credential (`aud=ziee-download`, 1h TTL) with
  no epoch. Narrow: single file, owner-scoped, perms re-checked at download. Cannot produce the
  reported symptom. Noted in STATUS.
- **change-password / admin password-reset** — still refresh-only revocation. Explicitly descoped in
  DEC-8: change-password needs a session re-mint in the response, which is real design work.

## Rejected (explicitly, not silently)

- **Forgery via stripping `ver`** — not possible: `ver` rides inside the signed JWT, `Validation::default()`
  pins HS256 and verifies the signature before `Claims` exist, and there is no dangerous-decode path
  in the tree. Audience separation blocks replaying a `ver: None` refresh token as an access token.
- **Reload loop** — the wipe precedes `reload()`, zustand persist writes localStorage synchronously,
  and the reloaded tab takes `initAuth`'s `if (!token) return`. Concurrent 401s collapse to one
  reload via `refreshSessionInFlight` + `navigator.locks`.
- **`clearedSession` regressions** — all four replaced sites verified correct; `{...clearedSession,
  ...baseError}` preserves the originals' semantics (baseError spreads last and wins).
- **Migration risk** — metadata-only on PG11+; no `SELECT *`/`RETURNING *`; every `INSERT INTO users`
  uses an explicit column list.

This round's fixes are re-verified by a FRESH blind round — see FIX_ROUND-2.

**New confirmed findings:** 8
