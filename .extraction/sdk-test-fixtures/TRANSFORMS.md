# Chunk `sdk-test-fixtures` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form. Files not listed
moved **BYTE-FOR-BYTE**: `oauth_mock.rs`, `ldap_mock.rs`, `apple_mock.rs`, and
the `apple_test_key.p8` blob (0 changed lines/bytes vs the pre-move file).

- **T-1** `sync_probe.rs` — `SyncProbe::open` signature genericised:
  `pub async fn open(server: &crate::common::TestServer, token: &str)`
  → `pub async fn open<S: crate::ApiUrlTarget>(server: &S, token: &str)`.
  **why:** the probe's ONLY app coupling was reading the `/sync/subscribe` URL
  off ziee's `TestServer` shim (`server.api_url("/sync/subscribe")`). Everything
  else already parses the SSE frames as raw `serde_json::Value`
  (`{entity, action, id}` → `String`), naming no app-side `SyncEntity` type. The
  new `crate::ApiUrlTarget` seam (a 1-method, dep-free trait: `fn api_url(&self,
  path: &str) -> String`) lets the probe build the URL without naming any
  app-side type. Body of `open` is otherwise unchanged; the emitted request +
  frame-parsing are identical for a fixed server → behaviour-preserving.

- **T-2** `ziee-test-harness/src/lib.rs` — ADDED (a) `#[cfg(feature =
  "fixtures")] pub mod fixtures;` and (b) the `pub trait ApiUrlTarget` (always
  compiled — dep-free — because its impl lives on the app side). **why:** the
  fixtures home + the sync-probe seam.

- **T-3** `ziee-test-harness/Cargo.toml` — ADDED 7 **optional** deps
  (serde_json, tokio-stream, testcontainers, wiremock, jsonwebtoken, base64,
  rsa) + a `[features] fixtures = [...]` that turns them on and additionally
  enables `reqwest/stream` (sync_probe's `bytes_stream()`) and `tokio/macros`
  (the fixtures' own `#[cfg(test)]` self-tests). **why:** keep the lean harness
  core dep-light; versions/feature-sets mirror the ziee server catalog so the
  single Cargo.lock unifies them with no duplicate build.

- **T-4** `src-app/server/tests/common/mod.rs` — the 4 `pub mod oauth_mock;` …
  declarations become re-export shims
  (`pub mod oauth_mock { pub use ziee_test_harness::fixtures::oauth_mock::*; }`)
  + a `impl ziee_test_harness::ApiUrlTarget for TestServer { fn api_url(&self,
  path: &str) -> String { self.api_url(path) } }`. **why:** preserve every call
  site's module path UNCHANGED + wire the probe seam. `self.api_url(path)` inside
  the trait impl resolves to `TestServer`'s **inherent** method (inherent wins
  over the same-named trait method — no infinite recursion).

- **T-5** `src-app/server/Cargo.toml` — the `[dev-dependencies]`
  `ziee-test-harness` path dep gains `features = ["fixtures"]`. **why:** the
  server integration tests consume the moved mocks; the desktop crate's harness
  dep stays feature-less (it references none of them).

## Fixtures-home Decision (recorded)
`ziee-test-harness` **`fixtures` feature-gated module** chosen over a sibling
`ziee-test-fixtures` crate: co-located with the harness per the task steer, and
the feature gate preserves the harness's LEAN/build-DB-free default posture while
isolating the heavy testcontainers/wiremock/rsa deps to opt-in consumers.

## sync_probe genericity finding (recorded)
**MOVED GENERIC.** `sync_probe` is wire-type-agnostic — it decodes each `sync`
frame with `serde_json::from_str::<serde_json::Value>` and lifts `entity`/
`action`/`id` as bare `String`s (see `sync_probe.rs` L77-90). It never names
ziee's `SyncEntity` enum or any app wire struct. The sole coupling
(`TestServer`, for the subscribe URL) is abstracted behind `ApiUrlTarget`. No
reason to leave it ziee-side.
