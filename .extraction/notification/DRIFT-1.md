# Chunk `notification` — DRIFT scan (round 1)

Drift = any place moving the notification types + permission key + migrations could
diverge from pre-extraction behavior / surface / output / schema. Each candidate
reconciled.

- **DRIFT-1.1** — verdict: none. **models.rs byte-identity.** Copied then `diff
  <(git show 4a2391732:…/models.rs) sdk/…/models.rs` is empty (exit 0). The row
  type, the `NewNotification` builder chain, and the paged/unread response types are
  unchanged.

- **DRIFT-1.2** — verdict: none. **Migration integrity (N7).** Both `.sql` moved
  byte-for-byte with ORIGINAL filenames + content, so sqlx checksums + version
  numbers are preserved; the app-side dir is deleted so each is globbed exactly once
  (via `sdk/crates/*`). The merged set reproduces ziee's exact `_sqlx_migrations`
  history; the deferred-FK band `4180` still sorts after every referenced table.
  Proven: ziee's build.rs provisioned the merged build DB (incl. these) and the 10
  `query_as!` verified (`cargo check -p ziee` exit 0).

- **DRIFT-1.3** — verdict: none. **repository stays → queries verify against the
  full DB.** The notification fkeys reference other modules' tables, so a standalone
  crate build DB would fail them (unlike self-contained ziee-auth). Keeping
  `repository.rs` app-side (verifying against the merged build DB) preserves the
  exact 10 `query_as!` behavior; `super::models` resolves the moved types.

- **DRIFT-1.4** — verdict: none. **events sync coupling preserved.** `events.rs`
  (which names the concrete `SyncEntity::Notification` + `publish`) stays app-side,
  so the `Audience::owner` / `origin=None` live-push on create is unchanged.

- **DRIFT-1.5** — verdict: none. **Shim transparency.** `mod.rs`'s `pub use
  ziee_notification::{models, permissions};` keeps `super::models`/`super::
  permissions` (repository/events/handlers) + `scheduler/dispatch.rs`'s
  `notification::models::NewNotification` resolving. ziee + ziee-desktop compile
  exit 0.

- **DRIFT-1.6** — verdict: none. **OpenAPI output (E8, BOTH surfaces).** The schema
  keys (`Notification`/`NotificationPage`/`UnreadCount`) + the `notifications::read`
  string (grant lives untouched in scheduler's migration) are unchanged.
  Regenerated ui + desktop: `types.ts` BYTE-IDENTICAL, `openapi.json`
  canonically-equal (jq -S) vs baseline. Restored via `git checkout`.

- **DRIFT-1.7** — verdict: none. **Boundary / build-hygiene.** ziee-notification
  names no app type + issues no `query!`; `FromRow` needs no DB → no build.rs, no
  per-crate build DB. `cargo check --workspace` exit 0. No new warnings.

**Unresolved drifts: 0**
