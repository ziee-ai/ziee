# PLAN_AUDIT — logout session invalidation

The plan audited against the codebase BEFORE writing code. Every claim below was checked against
real files in this worktree (base `origin/khoi` @ 44502c2c6), not inferred.

## Breakage risk

**Does any item break an existing caller?**

- **`generate_access_token` signature change (ITEM-3) — contained.** VERIFIED: it is a **private**
  fn (`fn generate_access_token`, no `pub`) with exactly **2 callers, both inside `jwt.rs`** (`:180`
  in `generate_tokens_with_jti_expiry`, `:217` in `reissue_tokens_for_jti`). Threading
  `token_version` through it cannot reach outside the module. Its 2 in-file `#[cfg(test)]` callers
  (`jwt.rs:388`, `:424`) need the new arg.
- **`mint_session_tokens` — zero caller churn.** The version is read INSIDE it, so its signature is
  unchanged and all 8 login-shaped call sites (register / login / LDAP / OAuth / link-account /
  first-run setup / desktop `auto_login` / tunnel password) compile untouched. This is the single
  design choice that keeps the diff small, and it is why desktop `auto_login` self-heals for free.
- **`revoke_all_for_user` — signature deliberately UNCHANGED.** `end_session_atomically` is added
  alongside it. Its 2 other callers (`auth/handlers.rs:906`, `user/handlers/user.rs:455`) are
  untouched. A tempting "tidy" refactor to make it transaction-aware would silently pull
  change-password into this diff — explicitly rejected (DEC-8).
- **`extract_authenticated_user` swapping `get_by_id` → `get_by_id_with_token_version` (ITEM-5)** —
  internal to `permissions/extractors.rs`; `get_by_id` itself keeps all its other callers.
- **Adding `origin: SyncOrigin` to `logout` (ITEM-8)** — `SyncOrigin` is `Infallible`
  (`sync/extractor.rs`), so it can never turn a working logout into an error; an absent header is
  just `None` (no self-echo suppression). Precedent: `user/handlers/user.rs:108,241,338`.
- **New 401 on a previously-passing request — INTENDED, and that is the whole feature.** The
  behavior change is bounded to tokens whose `ver` no longer matches, i.e. only after a logout.
  Pre-existing tokens carry no `ver` → `unwrap_or(0)` → match the `DEFAULT 0` column → keep working
  (TEST-11). So the deploy forces **zero** logouts.
- **Client (ITEM-10/11):** `tearDownSession` is additive; the desktop path is guarded by the
  pre-existing `refreshFallback` check, which is NOT moved. Adding `permissions: []` to wipe sites
  can only shrink state that should already have been cleared.

## Pattern conformance

| Item | Reference mirrored | Conforms |
|---|---|---|
| ITEM-1 migration | `00000000000064_add_users_password_changed_at.sql` (same table, `ADD COLUMN IF NOT EXISTS` + `COMMENT ON COLUMN`) | yes |
| ITEM-2 repo reads | `user/repository.rs::get_by_id:24-39` (explicit column list, `fetch_optional`, `AppError::database_error`) | yes |
| ITEM-3 optional claim | `jwt.rs::Claims.jti:19-25` (`#[serde(default, skip_serializing_if = "Option::is_none")]`) | yes |
| ITEM-6 transaction | `refresh_tokens.rs::claim_rotation_and_register:163-206` — SAME FILE: `pool.begin()` → `execute(&mut *tx)` → `tx.commit()` | yes |
| ITEM-7 read in mint path | `refresh_tokens.rs::session_expiries:19-33` (mint path already does DB I/O) | yes |
| ITEM-8 sync publish | `user/handlers/user.rs:461-469` (`SyncEntity::Session` / `Update` / `Audience::owner` / `origin.0`) | yes |
| ITEM-10/11 client | existing `endSession()` + `sessionEpoch` guard reused verbatim; no new mechanism | yes |
| Tests | `tests/sync/delivery_test.rs:126-160` (SyncProbe); `tests/workflow_mcp/mod.rs:131` (direct pool); `ChatHistory.store.test.ts:18-40` (vitest + `vi.mock`) | yes |

**Deviation, deliberate:** DEC-11 makes the version check **fail-closed** on a DB error, whereas the
sibling `session_expiries` fails *open* (falls back to config). Justified: that is a lifetime lookup,
this is an authorization gate.

## Migration collisions

- Highest on base: **157**. This branch takes **158** — free at plan time (verified `ls migrations | tail -1`).
- **This repo has a real history of collisions**: `00000000000155` was concurrently used by
  `155_create_voice_models` and `155_scheduled_task_run_result_preview`; one was renumbered to 156.
  So a 158 landing on khoi before merge is a live possibility, re-checked by `merge-gate.mjs` (C2).
- Operational note: there is **no `.sqlx` offline dir**, so the dev/build DB must have 158 applied
  before `cargo build`, and `cargo clean` is required for `build.rs` to re-run migrations. Budgeted.

## OpenAPI regen

**Not required — VERIFIED EMPIRICALLY, not assumed.**

- The `ver` claim lives *inside* the JWT, which is an opaque `String` in `TokenPair`. No
  request/response schema changes.
- `token_version` is kept OFF the `Serialize + JsonSchema` `User` struct (DEC-6), so `MeResponse` and
  the Tauri `AutoLoginResponse` are unchanged.
- **The `SyncOrigin` question was checked, not hand-waved:** `toggle_user_active` ALREADY takes
  `origin: SyncOrigin`, and its committed spec entry is `POST /api/users/{user_id}/toggle-active
  params=<none>` — identical to `POST /api/auth/logout params=<none>`. ⇒ `SyncOrigin` contributes no
  parameters, so adding it to `logout` cannot perturb `openapi.json`.
- ⇒ `just openapi-regen` is NOT needed for `ui/` or `desktop/ui/`. **`just openapi-check` and
  `openapi::emit_ts::tests::types_ts_parity` must stay green; if either goes red, a type leaked into
  the public surface — investigate, do not regen.**

## Per-item verdicts

- **ITEM-1** — verdict: PASS — 158 is free on base; mirrors migration 64's shape on the same table; no index needed (PK-only reads).
- **ITEM-2** — verdict: PASS — additive; mirrors `get_by_id`'s explicit-column-list idiom; keeps the column off the serializable `User` per DEC-6.
- **ITEM-3** — verdict: PASS — `generate_access_token` verified private with 2 in-file callers; `jti` is the exact optional-claim precedent; `#[serde(default)]` gives the zero-forced-logout deploy (TEST-11).
- **ITEM-4** — verdict: PASS — `grep "impl.*FromRequestParts"` confirms exactly 4 JWT extractors and 2 gating validation paths, so 2 call sites are provably total. Including `OptionalJwtAuth` (dead today) prevents leaving an unchecked path in-tree.
- **ITEM-5** — verdict: PASS — folds into the query the hot path already issues (`extractors.rs:62-85`), so `RequirePermissions` routes gain **no** round-trip. Ordering the version check BEFORE the `is_active` check is arbitrary but harmless (both 401/403 terminally).
- **ITEM-6** — verdict: PASS — `claim_rotation_and_register` in the same file is an exact structural precedent. Closes the human's review-fix-1 window.
- **ITEM-7** — verdict: PASS — `session_expiries` establishes that the mint path already does DB I/O; reading one more scalar there costs nothing and keeps all 8 callers unchanged.
- **ITEM-8** — verdict: PASS — `SyncOrigin` is `Infallible` and spec-neutral (verified above); publish-after-commit is required so a tab racing on the signal observes the bump.
- **ITEM-9** — verdict: PASS — a pure reordering inside `refresh`; closes window 2 of the same invariant. Correct under READ COMMITTED for both interleavings (a `SELECT` sees the pre-bump value; the claim blocks on the row lock).
- **ITEM-10** — verdict: CONCERN — `window.location.reload()` is a blunt instrument and is **untestable in a pure node:test unit** — it REQUIRES the jsdom/vitest runner (`vi.stubGlobal`) plus the e2e specs. Mitigated: `vitest.config.ts` already exists for exactly this (jsdom + `vi.mock('@/api-client')`), and TEST-14..17 + TEST-19..21 cover it. Reload-loop risk analyzed: the wipe writes `{token:null}` to `auth-storage` synchronously BEFORE reloading, so `initAuth`'s `if (!token) return` early-out breaks the loop after at most one extra reload. Accepted (DEC-3, human-approved).
- **ITEM-11** — verdict: PASS — a pure state-hygiene fix at 4+1 sites; strictly reduces retained state. `hasPassword` was found omitted alongside `permissions` (the brief named only `permissions`).
- **ITEM-12** — verdict: PASS — the comment at `tests/auth/mod.rs:330-332` is the codebase's own acknowledgement of the bug; replacing it with a real assertion is required or the suite keeps documenting the vulnerability as intended behavior.

- **ITEM-13** — verdict: CONCERN — *(added in DRIFT-1)* Fixes a regression the plan would otherwise have shipped: the built-in-MCP internal JWT is a third `Claims` site outside `jwt.rs`, and a defaulted `ver` would 401 every built-in tool call for any user who had ever logged out. The CONCERN is scope, not correctness — it makes `inject_builtin_context_headers` **async**, which is a signature change on a shared helper. Mitigated: only **2 call sites** (`manager.rs:145`, `mcp/handlers/test_connection.rs:259`), both already in async fns, so no ripple; and `generate_short_lived_jwt` stays sync (the caller passes the epoch in). Not optional — omitting it breaks memory/files/web_search/code_sandbox/citations/knowledge_base for logged-out-once users. Covered by TEST-22, which extends the existing enumerate-EVERY-built-in probe test so future built-ins inherit it.

- **ITEM-14** — verdict: PASS — *(added in DRIFT-2, from the blind audit)* Closes a HIGH that made the whole feature bypassable: without the `users` FOR SHARE lock, READ COMMITTED lets a successor refresh token INSERTed-but-not-COMMITTed escape logout's revoke scan entirely, then re-mint a valid epoch-N+1 session. Deadlock risk assessed and rejected: the lock is taken FIRST so both paths order `users → refresh_tokens`; `UPDATE users` takes FOR NO KEY UPDATE (conflicts with FOR SHARE, as required — `FOR KEY SHARE` would NOT have); concurrent refreshes take compatible FOR SHARE; the successor INSERT's FK check wants a weaker lock already held; change-password/admin-reset are autocommit, so no cycle. Independently re-derived by the round-2 auditor. Deterministically covered by TEST-25 (negative-controlled).
- **ITEM-15** — verdict: PASS — *(added in DRIFT-2, from the blind audit)* Free at the point of use: both SSE re-check loops already call `get_by_id` every tick, so swapping in `get_by_id_with_token_version` adds a column, not a round-trip. Fixes a real 24h hole — the chat stream keeps delivering assistant CONTENT past logout, and the client-side `Session` fan-out is no boundary against a holder of a stolen token. Verified it cannot kill a legitimate stream: `ver == None` can only reach the re-check via a `RequirePermissions`-gated subscribe (so `claims` is always `Some` and a `ver`-less token implies db == 0); a vanished user row already breaks; a DB error already keeps the stream. Covered by TEST-24 (sync); the chat-stream arm is line-for-line symmetric but untested — recorded in FIX_ROUND-2/STATUS rather than papered over.

**No BLOCKED verdicts.** Two CONCERNs (ITEM-10, ITEM-13), both mitigated; ITEM-10 human-approved.
