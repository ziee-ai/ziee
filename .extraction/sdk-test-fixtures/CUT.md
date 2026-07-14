# Chunk `sdk-test-fixtures` — 4 generic auth/sync test fixtures → SDK (MOVE)

Extract the 4 GENERIC backend test fixtures out of the ziee server crate's
`tests/common/` into the SDK, co-located with the test harness, so a second app
can drive the same auth/sync flows against them. All four carried **0 `ziee::`
references**. A small, clean, equivalence-preserving MOVE.

## Fixtures-home decision
Added a **`fixtures` module to the existing `ziee-test-harness` crate**, gated
behind a new `fixtures` cargo feature (NOT a sibling `ziee-test-fixtures` crate).
Rationale: keep them WITH the harness (the task's steer); the harness is
deliberately LEAN (runtime-only Postgres, build-DB-free) and these fixtures pull
heavier test-only deps (testcontainers, wiremock, jsonwebtoken, rsa, base64,
tokio-stream, serde_json) — so the feature gate keeps the default harness
dep-light while letting a consumer opt in with `features = ["fixtures"]`. Only
the ziee **server** crate opts in; the desktop crate keeps the bare harness.

## MOVE lines (git mv — history preserved)

| Fixture | lines | from | to |
|---|---|---|---|
| `oauth_mock.rs` | 161 | `src-app/server/tests/common/` | `sdk/crates/ziee-test-harness/src/fixtures/` |
| `ldap_mock.rs`  | 152 | `src-app/server/tests/common/` | `sdk/crates/ziee-test-harness/src/fixtures/` |
| `apple_mock.rs` | 174 | `src-app/server/tests/common/` | `sdk/crates/ziee-test-harness/src/fixtures/` |
| `sync_probe.rs` | 223 | `src-app/server/tests/common/` | `sdk/crates/ziee-test-harness/src/fixtures/` |
| `apple_test_key.p8` | (241 B) | `src-app/server/tests/fixtures/` | `sdk/crates/ziee-test-harness/tests/fixtures/` |

- `oauth_mock` / `ldap_mock` — **byte-identical** (pure relocation). Their only
  deps are external-endpoint crates (testcontainers, reqwest, tokio, serde_json).
- `apple_mock` — **byte-identical source**; travels WITH its `apple_test_key.p8`
  fixture because it resolves that file via `env!("CARGO_MANIFEST_DIR")` +
  `tests/fixtures/apple_test_key.p8`. Inside the harness crate CARGO_MANIFEST_DIR
  now points at the harness, so placing the `.p8` under the harness's
  `tests/fixtures/` keeps `fixture_p8_path()` correct AND self-consistent for
  every consuming app (the fixture file lives beside the fixture code). This is
  the one legitimate manifest-relative use — the opposite situation from the
  harness's spawn engine (which must never use CARGO_MANIFEST_DIR). Only
  `tests/auth/apple_test.rs` reads it, via `AppleMockServer::fixture_p8_path()`.
- `sync_probe` — moved with a **single source edit** (genericity, see TRANSFORMS
  T-1). It is truly generic: it parses the SSE stream as **raw JSON**
  (`{entity, action, id}` → `String`s), naming NO app-side `SyncEntity`/wire
  type. Its only app coupling was `&crate::common::TestServer` (for the
  `/sync/subscribe` URL) → now generic over a `crate::ApiUrlTarget` seam.

## STAYS app-side (NOT moved — domain fixtures)
`stub_chat.rs`, `stub_engine.rs`, `oai_capture_stub.rs` (LLM provider/engine
stubs — AI-domain), `chat_stream_probe.rs` (chat-specific). `harness_inner.rs`
was already SDK-backed (prior `ziee-test-harness` chunk) and is unchanged here.

## ziee stays a thin consumer
`src-app/server/tests/common/mod.rs`: the 4 `pub mod X;` become re-export shims
(`pub mod oauth_mock { pub use ziee_test_harness::fixtures::oauth_mock::*; }` …)
so every call site — `crate::common::oauth_mock::OAuthMockServer`,
`crate::common::sync_probe::SyncProbe::open(&server, &token)`, etc. — compiles
UNCHANGED across `tests/auth/*` and `tests/{user,llm_provider}/sync_emit_test.rs`.
Plus a 3-line `impl ziee_test_harness::ApiUrlTarget for TestServer` (the sync
probe seam). The desktop test binary shares `#[path]harness_inner.rs` as its
`common` (NOT `mod.rs`) and references NONE of the 4 fixtures → untouched.

## Submodule protocol
`sdk` is a git **submodule** (`ziee-ai/sdk`, gitlink mode 160000), not a subtree
— so the fixture files are committed INSIDE the sdk repo (detached HEAD, matching
the prior chunk's pattern); the ziee superproject then stages the `src-app`
deletions + Cargo/mod edits + the bumped `sdk` gitlink. NOT pushed.

## Gates (ALL green)
- `cargo check -p ziee` = **0** (2 pre-existing unrelated warnings).
- `cargo check -p ziee-desktop` = **0**.
- `cd sdk && cargo check --workspace` = **0** (default features); harness with
  `--features fixtures` also checks clean.
- `cargo test --test integration_tests --no-run` = **0** — the shim resolves.
- Equivalence: `sync::` = **17 passed / 0 failed**; `auth::oauth` + `auth::ldap`
  = **28 passed / 0 failed**; `auth::apple` = **9 passed / 0 failed** (exercises
  `fixture_p8_path()` → the relocated `.p8`). 54 tests through the moved
  fixtures, 0 failures.
- No Rust-API / OpenAPI / generated-`types.ts` impact (test-only move) — golden
  untouched by construction (no handler/model/route touched).
