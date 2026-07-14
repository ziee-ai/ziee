# Chunk B6 — BOUNDARY

What the framework's new `openapi` module may and may not name, and why the move
keeps the app-agnostic boundary clean.

## The framework openapi module is domain-free

- `emit_ts.rs` deps: `indexmap`, `serde`, `serde_json`, `std::fmt`. **No** app
  types, **no** DB, **no** `Config`, **no** `ziee_core`/`ziee_identity` domain
  types. It is a pure `&str (openapi.json) → String (types.ts)` transform.
- `mod.rs::finish_and_emit` deps: `aide::axum::ApiRouter`, `aide::openapi::OpenApi`
  (already framework deps from B2's `app_builder`), `serde_json`, `std::{fs,path}`.
  It takes an already-finished router + doc + two output paths — no app types.
- Grep confirms: no `crate::modules`, no app `Config`, no domain crate reference
  in either file.

## What stayed app-side (the boundary line)

The generation driver **head** names app-specific types and stays in ziee:
- `crate::core::config::Config` (the app config) — loaded + its data-dir/caches
  published into process globals.
- The ziee module set (`app_builder::create_modules` + per-module `init`).
- `set_app_data_dir` / `set_caches_config` / `init_repositories` (process globals
  the module `init` hooks read).
- Desktop only: `create_desktop_modules` + `build_desktop_api_routes` +
  `server_router.merge(desktop_router)` (the combined-spec assembly).

This mirrors the seam of every prior chunk: the framework owns the app-agnostic
machinery (module system B2, enforcement B3, repositories B4, sync core B5, now
the codegen tail + generator B6); the app owns the config-bound composition. An
app supplies its OWN config + module set + output paths and gets the identical
generator + emit pipeline.

## Second-consumer proof

`sdk/examples/skeleton-server` (depends on ONLY `ziee-core` + `ziee-framework`,
zero ziee domain) still `cargo check`s. It can now reach the generator + tail via
`ziee_framework::openapi::{emit_ts, finish_and_emit}` — the executable definition
of "a fresh app builds its own `types.ts` via the SDK generator" (plan §4).

## Output stays per-app

The generated `openapi.json` + `types.ts` are per-app OUTPUT, written to app-owned
paths supplied at the call site. The framework hosts the GENERATOR; the app owns
the ARTIFACTS and their locations. `E8` byte-identity of `types.ts` proves the
relocation changed no output.
