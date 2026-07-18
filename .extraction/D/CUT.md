# Chunk D — CUT manifest

Chunk D is **PARTIAL** (see `STOP_REPORT.md`). No live app code is deleted from
ziee this chunk — the reusable-shell MOVE is blocked on the BG-3 prerequisite,
and forcing it would break the tree (the harness cannot reference `ziee::`).
This CUT records the SDK-side design types created + the app-side/blocked map.

## Created in `sdk/desktop/harness` (green, SDK-only deps)

| File | Symbols | Purpose |
|---|---|---|
| `Cargo.toml` | crate `ziee-desktop-harness` (deps: ziee-core/identity/auth + anyhow/async-trait/serde/sqlx/uuid/tracing) | SDK-only deps; NO `ziee`, NO tauri yet |
| `src/lib.rs` | module wiring + re-exports | crate root |
| `src/manifest.rs` | `DeploymentMode`, `CapabilityManifest`, `FrontendManifest` | design-gate 1 (4-part manifest) |
| `src/single_user.rs` | `SingleUserStrategy`, `OwnerLogin`, `OWNER_WILDCARD_PERMISSION`, `mint_owner_login`, `owner_missing`, `owner_permissions` | design-gate 2 (single-user + owner-`*`) |
| `src/boot.rs` | `ServerBoot` trait, `BootHandle` | the BG-3 seam (embed-server boundary) |

## STAYS app-side (unchanged this chunk; per Chunk D "stays app-side")

- `core/module_builder.rs::create_desktop_modules` (the module vec).
- Desktop-only modules: `remote_access`, `magic_link`, `tunnel_auth`,
  `host_mount`, `tray`, `updater`.
- `backend/mod.rs:147-169` feature-flag overrides (sandbox/bio_mcp/web_search on).
- `create_desktop_config` YAML template + CORS allowlist + branding.
- `ui/src/modules/loader.desktop.ts` `CORE_MODULE_BLOCKLIST` **contents**.
- The app-side admin CRUD `create_admin_user` + Administrators-`*` grant (BA kept
  domain admin CRUD app-side).

## BLOCKED on BG-3 (moves to the harness only after the seam is threaded)

- `lib.rs::run` / `run_headless`, `register_desktop_invoke_handler`.
- `backend/mod.rs::{start_backend_server, create_main_window}` + the JWT/config
  `OnceLock`s.
- `auth/commands.rs::{mint_admin_login, auto_login}` +
  `auth/bootstrap.rs::ensure_desktop_admin` (reach global `ziee::Repos`).
- The 2 Tauri commands `get_server_port` + `auto_login`.
- Embedded-PG connect/start out of `server/core/database/mod.rs:128-429` →
  `ziee-framework` DB bootstrap (parameterized over the app's `sqlx::migrate!`).

## E5/E6 note

E5 (every CUT file exists in the SDK) holds for the created design types. E6
(deleted-from-ziee) is **N/A** this chunk — nothing was cut from ziee, so there
is no divergent duplicate to reconcile; the design types are net-new SDK
surface, not a move of existing app code.
