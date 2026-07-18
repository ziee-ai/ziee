# Chunk D — STOP_REPORT: the reusable-Tauri-shell MOVE is blocked on a BG-3 prerequisite

**Verdict: PARTIAL.** The two design-gates (capability manifest + single-user
strategy/owner-`*`) and the boot seam are delivered as green SDK-side types in
`ziee-desktop-harness`. The **live Tauri-shell MOVE** (`run`/`run_headless`,
the 2 IPC commands, `create_main_window`, the embed-server spawn) and the
**embedded-Postgres relocation** are **STOPPED**, not forced, because they hit
exactly the deep global coupling the fail-safe names. This is the direct analog
of BA's STOP_REPORT: the move needs a de-globalization chunk first.

## The blocker — one coupling, deep, not cleanly injectable in this chunk

The reusable harness **cannot reference `ziee::`** (it must build in the SDK
workspace so a second app — CytoAnalyst — consumes it). But every reusable piece
of the live shell bottoms out in the app crate:

| Live shell piece (app-side today) | Reaches | SDK-reachable? |
|---|---|---|
| `start_backend_server` / `run_headless` embed-server spawn | `ziee::start_server_with_routes` | **NO** |
| auto-login command / `mint_admin_login` | `ziee::Repos.user`, `ziee::Repos.pool()` | **NO** (global `Repos`) |
| `ensure_desktop_admin` | `ziee::Repos.user.has_admin`, `ziee::Repos.app.create_admin_user` | **NO** (global `Repos` + app-side admin CRUD) |
| JWT stash for the `auto_login` command | `JWT_SERVICE: OnceLock<Arc<JwtService>>` | app-owned static |
| CORS re-layer on merged routes | `ziee::create_cors_layer` | app-side helper |
| shutdown | `ziee::cleanup_server` | app-side (DB globals) |

The deepest is `start_server_with_routes` → `setup_server` (`server/lib.rs:426`):
it is the **app's entire server assembly** — `core::init_repositories` (the
`Repos` global), `core::app_builder::create_modules()` (the app module vec:
chat/LLM/memory/…), `ZieeIdentityResolver`, `build_auth_context`, and
`control_mcp::catalog::init_from_openapi`. None of that is app-agnostic, so it
**cannot move into a reusable crate**; the harness must receive a booted server
through an injected seam.

BG de-globalized the **auth module's** internal `Repos`/JWT/config use (behind
`AuthContext`), which unblocked BA. BG did **not** de-globalize the **desktop
consumer** surface: `desktop/tauri` still calls `ziee::Repos.*` and
`ziee::start_server_with_routes` directly. That is the unbuilt prerequisite.

## The prerequisite — "BG-3": thread the `ServerBoot` seam from the app

Introduce, in `ziee-desktop` (and the thin app glue), an impl of the harness's
already-defined seam **`ziee_desktop_harness::boot::ServerBoot`**:

```rust
#[async_trait] pub trait ServerBoot {
    async fn boot(&self) -> anyhow::Result<BootHandle>;   // BootHandle { addr, pool, jwt }
    async fn shutdown(&self);
}
```

The app's impl calls its own `start_server_with_routes` with the desktop route
re-layering closure (CORS + `Extension(jwt)`), runs desktop migrations, ensures
the owner via its app-side `create_admin_user` + Administrators-`*` grant, and
returns `{ addr, pool, jwt }`. Concretely BG-3 must:

1. Replace the app's `JWT_SERVICE` / `BACKEND_CONFIG` / `BACKEND_STATE`
   `OnceLock`s + the direct `ziee::Repos.*` reads in `auth/{commands,bootstrap}.rs`
   and `backend/mod.rs` with the `BootHandle` the seam returns (pool + jwt
   threaded, not global-fetched).
2. Keep the app-specific bits app-side (per Chunk D's "stays app-side"):
   `create_desktop_modules`, the desktop-only modules, the CORS allowlist +
   feature overrides + branding, `create_admin_user`.
3. Relocate the **generic** embedded-PG connect/start out of
   `server/core/database/mod.rs` into **`ziee-framework`'s DB bootstrap**,
   parameterized over the app's `sqlx::migrate!` (which is schema-bound and stays
   app-side — same rule as `ziee-auth`'s `AUTH_MIGRATOR`). The `DATABASE_POOL` /
   `POSTGRESQL_INSTANCE` `OnceCell`s currently in the server crate move with it
   (or behind a handle) so `cleanup_server` still works.

Only after (1)–(3) can `run`/`run_headless` + the 2 commands + `create_main_window`
move into `ziee-desktop-harness` generic over `Arc<dyn ServerBoot>` — a clean,
equivalence-preservable MOVE at that point.

## Why STOP instead of forcing it here

- Doing (1)–(3) as one chunk is a large, invasive rewrite of the **live desktop
  boot path** (window-close cleanup, headless test parity, the JWT-secret
  per-boot policy, the CORS re-layer, migration ordering) — an equivalence
  regression here would be silent until desktop runtime.
- The verifying gate for that MOVE is the **desktop E2E** (permanent-session +
  `auto_login` + the 2 IPC commands), which cannot be run in this Bash-tool
  harness. Shipping an unverifiable rewrite of the auth-carrying boot path
  violates the "don't force a broken tree" mandate and the security posture of a
  single-user auto-login surface.
- The fail-safe explicitly authorizes a BG-3-style STOP for exactly this
  coupling (`ziee::Repos` / JWT `OnceLock` / config statics / the
  `"admin"`/`is_admin` assumption).

## What WAS delivered (clean, green, equivalence-safe)

- The FOUR-part capability manifest (`manifest.rs`) — design-gate 1.
- The single-user strategy + owner-`*` (`single_user.rs`), built on `ziee-auth`'s
  `mint_session_tokens` + `UserRepository` and `ziee-identity`'s `"*"` RBAC —
  design-gate 2. `mint_owner_login` / `owner_missing` are concrete + reproduce
  the app's `mint_admin_login` / `ensure_desktop_admin` semantics.
- The `ServerBoot` seam (`boot.rs`) — the BG-3 target, fully specified.
- Zero app-side edits ⇒ golden byte-identical/canonical **by construction**
  (nothing in the app graph depends on the harness yet).
