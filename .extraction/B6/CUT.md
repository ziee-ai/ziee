# Chunk B6 — emit_ts generator + openapi driver TAIL (CUT manifest)

Move the OpenAPI→TypeScript client generator (`emit_ts.rs`, a **pure function**
of the spec, ~1323 lines) and the app-agnostic **tail** of the generation driver
(`finish_api → serialize openapi.json → emit_ts → write types.ts`) from ziee's
`src-app/server/src/openapi/` into a new `ziee_framework::openapi` module. The
currently-hardcoded output path (`../src/api-client/types.ts`) is **parameterized**
(the app supplies it at the call site). The generator's OUTPUT (`types.ts`) MUST
stay **byte-identical** — the whole point of the chunk.

## Design gate — codegen

The generator is a pure `spec → String` transform (deps: `indexmap` + `serde` +
`serde_json` only; no DB, no app types). Its output is the real client contract
(`E8`: `types.ts` byte-identical; `openapi.json` canonically-equal via `jq -S`).
Relocating it must be output-preserving. The GENERATOR-CORRECTNESS golden test
moves to the SDK as a fixture test (a small committed `openapi.json` → a committed
expected `types.ts`, byte-identical); the per-app REGEN-DRIFT guard stays in ziee.
See TRANSFORMS `## Decision` for the golden-test split + the output-path
parameterization.

## Files — MOVED INTO `ziee-framework` (submodule `sdk/`)

- new: `crates/ziee-framework/src/openapi/emit_ts.rs` — the generator, moved
  verbatim (production code lines 1–1237 byte-for-byte). Test module replaced:
  keeps the 2 pure unit tests (`escapes_string_literals`,
  `quotes_non_identifier_member_keys`) + a NEW `generator_golden_fixture` test
  (fixture-based, byte-identical). Drops the ziee-coupled parity tests (they move
  to ziee).
- new: `crates/ziee-framework/src/openapi/mod.rs` — `pub mod emit_ts;` + the
  parameterized driver TAIL `finish_and_emit(router, api_doc, output_dir,
  types_ts_path)` (finish_api → openapi.json → emit_ts → types.ts).
- new: `crates/ziee-framework/src/openapi/fixtures/openapi.json` — the small
  golden-fixture spec (paths w/ path+query params, a 403 permission example, an
  interface schema w/ required/optional/nullable/array fields + doc-comments, an
  enum, a request body).
- new: `crates/ziee-framework/src/openapi/fixtures/types.ts` — the committed
  expected generator output for that fixture (produced by the current generator).
- edit: `crates/ziee-framework/src/lib.rs` — `pub mod openapi;`.
- edit: `crates/ziee-framework/Cargo.toml` — add `indexmap` (features `serde`) +
  promote `serde_json` from dev to a normal dependency (the generator + tail need
  them at runtime).

## Files — CHANGED IN ziee (submodule `src-app/`, NOT committed here)

- del: `src-app/server/src/openapi/emit_ts.rs` (moved to the framework).
- edit: `src-app/server/src/openapi/mod.rs` — `generate_openapi_spec` keeps its
  app-specific head (Config load, process globals, module create+init,
  build_api_router) and hands the finished `(ApiRouter, OpenApi)` to
  `ziee_framework::openapi::finish_and_emit(...)` with ziee's output paths
  (`output_dir` + `output_dir/../src/api-client/types.ts`). Hosts the moved
  per-app REGEN-DRIFT guard (`types_ts_parity` + `types_ts_parity_desktop`).
- edit: `src-app/server/src/lib.rs` — the two crate-root re-exports
  (`generate_types_ts_from_json`, `finish_and_emit`) now source directly from
  `ziee_framework::openapi::*`, so `ziee::generate_types_ts_from_json` +
  `ziee::finish_and_emit` (used by the desktop binary) resolve unchanged.
- edit: `src-app/desktop/tauri/src/openapi.rs` — builds the combined
  (server+desktop) router as before, then calls `ziee::finish_and_emit(...)`
  with desktop's output paths instead of the inlined finish/serialize/write.

## Stays app-side (each app owns)

The generation driver HEAD: loading the app `Config`, publishing the process
globals module `init` reads (`set_app_data_dir` / `set_caches_config` /
`init_repositories`), instantiating + initializing the module set, and — desktop
only — merging the desktop routes into the server router. These are app-specific
(they name the app `Config` + the app module set); they feed a finished router +
doc into the shared framework tail.
