# BASE — conflict-surface scoping (P3)

Base: `origin/khoi` @ `44502c2c6` ("Merge remote-tracking branch 'origin/main' into khoi").
PR target: **`khoi`** (human-confirmed at plan review; overrides the task file's "main").

> Note: `origin/khoi` is currently **1 commit BEHIND `origin/main`** — do not assume they are equal.
> All lifecycle gates run with `--base origin/khoi`.

## Migration numbers

- Highest existing on base: **`00000000000157_remove_unused_builtin_mcp_servers.sql`**.
- This branch takes **158** (`00000000000158_add_users_token_version.sql`). One migration only.
- **Collision risk is REAL and has bitten this repo before**: `00000000000155` was used *twice*
  concurrently (`155_create_voice_models` + `155_scheduled_task_run_result_preview`) and one was
  renumbered to 156. If any branch lands a 158 on khoi before this merges, renumber to the next free
  slot and re-run `build.rs` (`cargo clean` is required for it to re-run migrations — there is no
  `.sqlx` offline dir, so the dev DB must have 158 applied before `cargo build`).
- Re-checked against real khoi at merge time by `merge-gate.mjs` (C2).

## Files this branch touches that base is also moving

| File / area | Base activity | Collision risk |
|---|---|---|
| `src-app/server/src/modules/auth/handlers.rs` | `c66cd5d76 fix(auth): root OAuth redirect_uri at configured https public origin` — touches the OAuth callback / redirect path | **Low.** We touch `logout` + `refresh`; that commit touches OAuth redirect construction. Different fns in the same file → textual-merge risk only, no semantic overlap. |
| `src-app/ui/src/modules/auth/**` | `login-setup-theme` series (`c432ef445`, `ab665ee86`, `a38e7676e`) — `AuthScreenLayout`, theme toggle, heading placement | **Low.** Those are `AuthPage`/layout components; we touch only `Auth.store.ts`. Already merged into base. |
| `src-app/server/src/modules/permissions/extractors.rs` | none recent | None. |
| `src-app/server/src/modules/auth/jwt.rs` / `jwt_extractor.rs` / `refresh_tokens.rs` | none recent | None. |
| `src-app/ui/tests/e2e/auth/logout.spec.ts`, `tests/e2e/sync/session-sync.spec.ts` | none recent | None (we append tests). |
| Gallery (`gallery/*.tsx`, 155-entry migration) | very active on base | **None — we add no gallery entry** (no new UI surface / render state). |

## OpenAPI regen

**Not implied.** The `ver` claim lives *inside* the JWT, which is an opaque `String` in `TokenPair`.
No request/response schema changes: `TokenPair`, `MeResponse`, `AuthResponse`, and `User` are all
untouched — specifically because `token_version` is deliberately kept OFF the `Serialize + JsonSchema`
`User` struct (it is read into a non-serialized internal row instead). `logout` stays `204`; adding
the `SyncOrigin` header extractor does not change its documented shape.

⇒ `just openapi-regen` is NOT needed for `ui/` or `desktop/ui/`; `just openapi-check` must stay green,
and `openapi::emit_ts::tests::types_ts_parity` should remain untouched. **If either goes red, that is a
signal a type leaked into the public surface — investigate rather than regen.**

## Desktop (R2-3)

`src-app/desktop/ui/` has **no `Auth.store.ts` override and no `core/permissions/*` override**
(verified: it overlays via `localOverridePlugin` and shadows only `main.tsx`, the codegen'd
`api-client/types.ts`, the memory module, and dev/gallery files). So the single `src-app/ui` fix
covers web + desktop, and no desktop file is edited. Desktop safety is preserved behaviorally via the
existing `refreshFallback` guard, not via an override.
