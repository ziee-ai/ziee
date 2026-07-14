# Chunk B6 — TRANSFORMS

Every transform applied moving the generator + driver tail into `ziee-framework`,
each with the design decision + resolution. Zero TBD.

## T-1 — `emit_ts.rs` production code moved VERBATIM

The generator's 1237 lines of production code (the `J` order-preserving value,
all render/emit fns, `generate_types_ts` / `generate_types_ts_from_json`) copied
**byte-for-byte** into `crates/ziee-framework/src/openapi/emit_ts.rs`. No logic,
ordering, sort, or escaping change. The only edits are to the `#[cfg(test)] mod
tests` block (T-4). Purity is intrinsic: the module's only imports are
`indexmap`, `serde`, `std::fmt` — no app types, no DB, no config.

**Resolution:** copied via `cp` then the test module was replaced; the production
region is diff-empty against the ziee original (confirmed by the byte-identical
regen — T-6 — which proves the relocated code produces identical `types.ts`).

## T-2 — driver TAIL parameterized as `finish_and_emit`

### Decision — how to split the driver, and what "parameterize the output path" means

`generate_openapi_spec` did: load Config → set globals → lazy pool → init repos →
create+init modules → `build_api_router` → **finish_api → serialize openapi.json
→ emit_ts → write types.ts**. The **head** (Config, globals, module set) names
app-specific types (`crate::core::config::Config`, the ziee module builder) and
CANNOT move to an app-agnostic framework. `create_modules` + `build_api_router`
already live in the framework (Chunk B2). The genuinely-shared, app-agnostic,
output-preserving remainder is the **tail** (bold above). The desktop binary must
inject its merged routes BETWEEN `build_api_router` and `finish_api`, so the tail
must accept an already-finished `(ApiRouter, OpenApi)` rather than re-run the
build. The hardcoded `output_path.join("../src/api-client/types.ts")` becomes an
explicit `types_ts_path: &Path` argument (plus `output_dir: &Path` for
`openapi.json`) — the app supplies both, so a second app emits to its own layout.

**Resolution:** framework fn
`finish_and_emit(router: ApiRouter, mut api_doc: OpenApi, output_dir: &Path,
types_ts_path: &Path) -> Result<(), Box<dyn Error + Send + Sync>>` runs
`finish_api → to_string_pretty → write <output_dir>/openapi.json → emit_ts →
write types_ts_path`, byte-for-byte the former inline sequence (same
`serde_json::to_string_pretty`, same `generate_types_ts_from_json`, same
create_dir_all guards, same `println!` lines). Both ziee (`output_dir` +
`output_dir/../src/api-client/types.ts`) and desktop (its own `output_dir` +
`../src/api-client/types.ts`) call it. The formerly-hardcoded relative derivation
now lives at each app's call site.

## T-3 — re-export surface (ziee unchanged for callers)

### Decision — where the re-exports live so no call site changes

Callers today: `main.rs` + `lib.rs:664` call `openapi::generate_openapi_spec`
(stays app-side, unchanged); `lib.rs:17` re-exports
`generate_types_ts_from_json` at the crate root (the desktop binary consumes it
as `ziee::generate_types_ts_from_json`); the desktop binary also needs the tail.
A transitive re-export through ziee's private `mod openapi` (`pub use
ziee_framework::openapi::{emit_ts, finish_and_emit}` in `openapi/mod.rs`, then
`pub use openapi::emit_ts::…` in lib.rs) tripped `-D unused-imports` on the
`emit_ts` module re-export (the transitive-`pub use` lint case).

**Resolution:** ziee's crate root re-exports BOTH symbols **directly** from the
framework — `pub use ziee_framework::openapi::emit_ts::generate_types_ts_from_json`
and `pub use ziee_framework::openapi::finish_and_emit` (lib.rs). `openapi/mod.rs`
then only `use`s the tail internally (used by `generate_openapi_spec`) and the
test references the generator via its full `ziee_framework::openapi::emit_ts`
path. `ziee::generate_types_ts_from_json` + `ziee::finish_and_emit` resolve
exactly as before for the desktop binary; zero unused-import warnings.

## T-4 / golden-test split — GENERATOR-CORRECTNESS → SDK; REGEN-DRIFT → ziee

### Decision — the golden test guarded two different things; split them

The old `types_ts_parity[_desktop]` tests (in `emit_ts.rs`) asserted
`generate(committed openapi.json) == committed types.ts` for ziee's `ui` +
`desktop/ui`. That conflated two properties: (a) **generator correctness** — the
generator is a faithful pure function (a property of the generator, app-agnostic);
and (b) **per-app regen-drift** — ziee's committed `types.ts` was actually
regenerated after its last spec change (a property of the ziee repo, needs ziee's
real committed artifacts). The generator now lives in the SDK, which must not
depend on ziee's committed files.

**Resolution — split:**
- **Generator-correctness → SDK.** A new `generator_golden_fixture` test in the
  framework's `emit_ts.rs` reads a small committed fixture
  (`src/openapi/fixtures/openapi.json`) and asserts the generator reproduces the
  committed `src/openapi/fixtures/types.ts` **byte-for-byte**. The fixture
  exercises the main code paths (interface schema w/ required/optional/nullable/
  array fields + doc-comments, an enum, path+query params, a 403 permission
  example, a request body). The expected `types.ts` was produced by the current
  (correct) generator, so any future change to the generator that alters output
  fails the test. The 2 pure unit tests (`escapes_string_literals`,
  `quotes_non_identifier_member_keys`) move alongside.
- **Regen-drift → ziee.** The `types_ts_parity` + `types_ts_parity_desktop` tests
  move verbatim into ziee's `openapi/mod.rs` `#[cfg(test)]`, calling
  `ziee_framework::openapi::emit_ts::generate_types_ts_from_json` against ziee's
  real committed `../ui` + `../desktop/ui` artifacts. The end-to-end driver test
  (`generates_spec_and_types_without_live_db`) also stays in ziee (it drives the
  app-specific `generate_openapi_spec`).

## T-5 — framework deps

### Decision — indexmap + serde_json become runtime deps

`emit_ts` deserializes into `indexmap`-backed `J` (NOT `serde_json::Value`, which
alphabetizes and would break insertion-order parity) and the driver tail
serializes the doc + reads the JSON back with `serde_json`. Neither was a
framework runtime dep (`serde_json` was dev-only).

**Resolution:** added `indexmap = { version = "2", features = ["serde"] }` and
promoted `serde_json = "1.0.141"` to `[dependencies]` in the framework
Cargo.toml — versions matching the ziee server catalog so the single
`src-app/Cargo.lock` unifies them (no duplicate build).

## T-6 — golden equivalence verified (E8, BOTH surfaces)

**Resolution:** see DRIFT-1 / FIX_ROUND-1 — regenerated ui + desktop with the
RELOCATED generator; `types.ts` (ui + desktop) **byte-identical** vs the committed
baseline; `openapi.json` (ui + desktop) **canonically-equal** (`jq -S`). Restored
via `git checkout`.
