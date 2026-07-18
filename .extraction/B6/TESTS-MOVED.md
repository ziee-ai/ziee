# Chunk B6 — TESTS-MOVED

## Moved INTO the SDK (`crates/ziee-framework/src/openapi/emit_ts.rs` tests)

| Test | Origin | Note |
|---|---|---|
| `escapes_string_literals` | ziee `emit_ts.rs` | pure unit, moved verbatim |
| `quotes_non_identifier_member_keys` | ziee `emit_ts.rs` | pure unit, moved verbatim |
| `generator_golden_fixture` | **NEW** (replaces the app-coupled parity test's generator-correctness role) | fixture-based: `src/openapi/fixtures/openapi.json` → committed `src/openapi/fixtures/types.ts`, **byte-identical**. The expected `types.ts` was produced by the current generator, so it catches future generator drift (not a tautology). |

SDK result: `cargo test -p ziee-framework openapi::emit_ts` → **3 passed**.

## Stayed in ziee (`src-app/server/src/openapi/mod.rs` tests) — the per-app REGEN-DRIFT guard

| Test | Note |
|---|---|
| `types_ts_parity` | asserts ziee's committed `../ui` `types.ts` == generator(committed `../ui/openapi/openapi.json`). Now calls `ziee_framework::openapi::emit_ts::generate_types_ts_from_json`. |
| `types_ts_parity_desktop` | same for `../desktop/ui` (combined server+desktop spec). |
| `generates_spec_and_types_without_live_db` | end-to-end drive of the app-specific `generate_openapi_spec` (lazy pool, module-init-continue-on-error, emits openapi.json + types.ts). Unchanged except the internal call now routes through `finish_and_emit`. |

## Split rationale (see TRANSFORMS T-4 / `## Decision`)

The old `types_ts_parity[_desktop]` guarded TWO properties: **generator
correctness** (app-agnostic — belongs with the generator, now in the SDK) and
**per-app regen-drift** (ziee re-ran `openapi-regen` — needs ziee's real committed
artifacts, so it stays app-side). The SDK fixture test owns the first; ziee's
retained parity tests own the second. The SDK must not depend on ziee's committed
files, which is exactly why the fixture is small + self-contained.
