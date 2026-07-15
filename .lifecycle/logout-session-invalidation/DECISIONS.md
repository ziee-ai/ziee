# DECISIONS — logout session invalidation

Every human/product input the implementation needs, resolved up front. Zero open questions.
DEC-2, DEC-3 and DEC-13 were escalated to the human as explicit option pickers (genuine product
choices); DEC-7 and DEC-6b came back from the human's plan review. The rest are resolved by
codebase convention with the rationale recorded.

### DEC-1: What mechanism revokes an already-issued access token — a `token_version` counter, a `sessions_revoked_at` timestamp compared to the existing `iat` claim, or a `jti` denylist?
**Resolution:** A per-user `users.token_version INTEGER NOT NULL DEFAULT 0` counter, stamped onto the access token as an optional `ver` claim and compared by **equality** in the extractors.
**Basis:** codebase — (a) a `jti` denylist is structurally impossible without new claims: access tokens carry `jti: None` (`jwt.rs:265`), so they are not individually addressable; (b) the `iat` timestamp design is **provably broken** — `iat` is minted as whole seconds (`jwt.rs:262`) while `NOW()` carries microseconds, so a token minted just BEFORE a logout and one minted just AFTER a re-login inside the same second are **bit-identical in `iat`**; `iat < revoked_at` therefore kills both (an infinite login loop for the rest of that second) and `date_trunc('second', …)` spares both (a 1-second hole). No operator separates them. A counter orders causally rather than temporally, so it has no time-granularity failure mode at all. Pinned by TEST-7.

### DEC-2: Does logout sign the user out on EVERY device, or only end the session that ran it?
**Resolution:** **Sign out everywhere** — a single per-user counter.
**Basis:** user — escalated as an option picker; the human chose "Sign out everywhere (Recommended)". This is not a new product behavior: `revoke_all_for_user` (`refresh_tokens.rs:264-277`) ALREADY revokes every refresh token the user holds on logout ("sign out everywhere"), and migration 44's own header documents that intent. The fix only removes the up-to-24h delay before that intent takes effect. The alternative (per-session) would require adding a `jti` to access tokens plus per-session tracking, and would contradict the existing refresh-token semantics.

### DEC-3: How is the client's per-user state torn down — reload the document, or sweep the store registry?
**Resolution:** **`window.location.reload()`** after wiping the auth state. No store-kit reset hook, no registry sweep, no edits to the other 122 store files.
**Basis:** user — escalated as an option picker (this diverges from the task brief, which prescribed wiring the ~50 unwired stores into a teardown); the human chose "Reload the document (Recommended)". Technically it is also the only correct option: a sweep loses two races it cannot win — an in-flight request issued with the previous user's token resolves AFTER the sweep and re-poisons the store, and the registry (`useModuleSystemStore.getState().stores`) does not own module-scope caches (`chatDrafts.ts`), React tree state, or the router. A reload discards all of it by construction.

### DEC-4: What carries the cross-tab logout signal — the existing SSE `Session` push, or a new BroadcastChannel / `storage` listener?
**Resolution:** Reuse the SSE `Session` push (`publish_session_to_users`). **No BroadcastChannel, no `storage` listener.**
**Basis:** convention — the task brief says "Coordinate with the existing session-sync / AUTH_EPOCH machinery — don't reinvent it." The fan-out already exists, is origin-suppressed, keyed per-user via the `by_user` index, and already fires on every admin-initiated session change (`user/handlers/groups.rs:250,430,482`; `user/handlers/user.rs:465,610`) — logout is the only caller missing. It also covers cross-DEVICE, which BroadcastChannel cannot. Residual gap (SSE dead AND tab idle) is stale pixels only, because DEC-1's server check is the backstop — pinned by TEST-21.

### DEC-5: Where does the version check go so that NO authenticated route is left open?
**Resolution:** Two call sites, one shared rule: `JwtAuth` (+`OptionalJwtAuth`) in `jwt_extractor.rs`, and `extract_authenticated_user` in `permissions/extractors.rs`. Both call the same pure `verify_token_version`.
**Basis:** codebase — there is **no auth middleware layer**; auth is 100% per-handler extractors. `grep "impl.*FromRequestParts"` yields exactly 4 JWT extractors: `JwtAuth`, `OptionalJwtAuth`, `RequirePermissions` and `RequireAdmin` — the latter two both funnel through `extract_authenticated_user`. The only other `validate_access_token` callers (`chat/stream/handler.rs:51`, `sync/handlers.rs:70`) read `exp` solely for a stream deadline and their routes are gated by `RequirePermissions`. ⇒ 2 call sites = provable 100% coverage. `OptionalJwtAuth` is unused today but is included so no unchecked validation path is left in-tree.

### DEC-6: Does `token_version` go on the `User` struct (free via the existing `get_by_id`)?
**Resolution:** **No.** It is read into a NON-serialized internal row by a dedicated `get_by_id_with_token_version`, plus a `get_token_version` scalar for the bare-`JwtAuth` path.
**Basis:** codebase — `query_as!(User, …)` uses explicit column lists in **11 verified sites** across `user/`, `auth/` and `app/repository.rs`, so a new field is an 11-file change; and `User` is `Serialize + JsonSchema`, embedded in `MeResponse` AND the Tauri `AutoLoginResponse`, leaving a session-secret-adjacent value one missing `#[serde(skip)]` away from a public response. Keeping it off `User` is also what keeps this a no-OpenAPI-regen change.

### DEC-6b: Should the version read cost an extra DB round-trip on every authenticated request?
**Resolution:** **No** — fold it into the query `extract_authenticated_user` already issues (`get_by_id_with_token_version` returns `(User, i32)`), so `RequirePermissions` routes stay at their current 2 queries. The 4 bare-`JwtAuth` handlers keep the dedicated scalar read (they do no `get_by_id` at all, so there is nothing to fold into).
**Basis:** user — the human's plan review: *"avoid a second DB round-trip per authenticated request by folding the token_version read into the existing get_by_id … via a NON-serialized internal struct — keep the column OFF the serializable User struct exactly as you planned; the 4 bare-JwtAuth handlers can keep the dedicated scalar read."*

### DEC-7: Must the logout bump + refresh-revoke be atomic?
**Resolution:** **Yes — one transaction** (`end_session_atomically`), with the `Session` publish only after `tx.commit()`. Additionally, `refresh` must read `token_version` BEFORE `claim_rotation_and_register`, closing the same invariant's second window.
**Basis:** user — the human's plan review caught this: *"otherwise a bump-succeeds-but-revoke-fails window lets a held refresh token re-mint a valid access token through mint_session_tokens (which reads the new ver) and defeats logout."* The invariant is: **no window in which a held refresh token can re-mint a valid access token past a logout.** Window 1 (partial logout) is closed by the transaction; window 2 (read-after-claim in `refresh`) by the ordering. Pinned by TEST-8 and TEST-9. Mirrors `claim_rotation_and_register:163-206`, the same file's existing atomicity shape.

### DEC-8: Which OTHER sites bump the counter — self-service change-password, admin password-reset, deactivate, delete-user?
**Resolution:** **None. Logout only.** Each is noted in the STATUS file as its own ticket.
**Basis:** convention — a bugfix's scope is bounded by causality (*can this produce the REPORTED symptom?*), not by *is it a real bug?*. Admin password-reset (`user/handlers/user.rs:455`) is a documented 1-line freebie whose own comment describes this exact gap, but it cannot produce the reported logout symptom. Self-service change-password (`auth/handlers.rs:906`) has the same symptom but needs a DIFFERENT fix — a bump would log the caller out mid-request; doing it right requires re-minting the caller's session in the response, which is real design work. Deactivate and delete-user are already closed at `permissions/extractors.rs:82` / a `None` user lookup. Corollary: `revoke_all_for_user`'s signature stays UNCHANGED — refactoring it to take a transaction would silently drag change-password into this diff.

### DEC-9: Does this feature introduce any operational tunable that should be an admin-configurable settings row?
**Resolution:** **No new tunable.** `token_version` is a correctness counter, not a knob — there is nothing for an operator to set. The one adjacent tunable, the access-token TTL, is ALREADY admin-configurable (`session_settings.access_token_expiry_hours`, migration 129, default 24h) and is deliberately not touched.
**Basis:** convention — the lifecycle's configurable-settings rule requires an explicit DEC for any limit/retention/toggle/threshold the feature adds. This feature adds none: no cap, no retention period, no rate limit, no feature toggle. Making revocation itself operator-disableable would be an anti-feature (a security boundary must not be weakenable by config), which is the rule's stated exception.

### DEC-10: What error code does a revoked-session request return?
**Resolution:** **401 `SESSION_REVOKED`.**
**Basis:** convention — mirrors the existing refresh-side vocabulary `REFRESH_TOKEN_REVOKED` (`session_refresh_test.rs:281`) and the `AppError::unauthorized("CODE", msg)` shape used by `INVALID_TOKEN` / `MISSING_TOKEN` / `USER_NOT_FOUND` (`jwt_extractor.rs:48`, `jwt.rs:322`, `permissions/extractors.rs:73`). 401 (not 403) is what the client's interceptor + `AUTH_REFRESH_EXEMPT` machinery already treats as a teardown signal.

### DEC-11: If the DB read for the version fails, does the request pass or fail?
**Resolution:** **Fail closed** — a 500, never an implicit pass.
**Basis:** convention — a revocation check that fails open is not a revocation check. Note this deliberately differs from `session_expiries` (`refresh_tokens.rs:19-33`), which falls back to config on error — that is a *lifetime* lookup where falling back is safe, whereas this is an *authorization* gate. It matches the fail-closed posture of the surrounding `get_by_id` → `USER_NOT_FOUND` 401 path.

### DEC-12: Does desktop's logout / permanent-session behavior change?
**Resolution:** **No.** `tearDownSession` is gated on `!refreshFallback` in `logoutUser`, and in `doRefresh` the existing `if (refreshFallback)` guard already precedes the teardown (do not move it). Desktop's current logout behavior is preserved exactly as-is, including its existing quirks.
**Basis:** codebase + convention — `AuthGuard.desktop.tsx` never renders a login page (the embedded server trusts the local user; `desktop-base/module.tsx:132-142` registers `auto_login` as the fallback). Reloading a Tauri window would strand it. Desktop self-heals from the bump for free because `auto_login` mints through the same `mint_session_tokens` (`desktop/tauri/src/modules/auth/commands.rs:36-45`). The ngrok phone surface has `refreshFallback === null` and correctly takes the web branch. Fixing desktop logout is explicitly NOT this ticket (R2-3: don't change desktop security behavior as a side effect). Pinned by TEST-15 and TEST-17.

### DEC-13: Which branch does this cut from, and which does the PR target?
**Resolution:** Cut from **`origin/khoi`**; PR targets **`khoi`**.
**Basis:** user — escalated because the task file says "branch `main`" while prior convention says `khoi`. The human's answer: *"Branch off khoi, then PR to khoi please — this matches our standard flow, do NOT change it to main."* Note `origin/khoi` is currently 1 commit BEHIND `origin/main`, so they are NOT interchangeable; all gates run `--base origin/khoi`.
