# Chunk `sdk-test-fixtures` — BOUNDARY

Coupling map of the 4 moved fixtures on the CURRENT base. Method: grepped every
`use` / `crate::` / cross-symbol reference in each fixture, plus every call site
across `src-app/server/tests/**` and `src-app/desktop/**`.

## External `crate::` surface of the moved set
```
 sync_probe.rs:   1  crate::common::TestServer   → genericised behind crate::ApiUrlTarget (T-1)
 oauth_mock.rs:   0
 ldap_mock.rs:    0
 apple_mock.rs:   0  (but 1 env!("CARGO_MANIFEST_DIR") self-reference — see below)
```
No `ziee::` anywhere. No `crate::modules::…`, no `AppError`, no `Repos`, no
`query!`. The auth mocks are pure external-endpoint fixtures (testcontainers /
wiremock + crypto libs). sync_probe's single coupling is the subscribe URL.

## Per-fixture deps (all test-only, all in the ziee catalog)
- `oauth_mock` — testcontainers, reqwest, tokio, serde_json.
- `ldap_mock`  — testcontainers, tokio, serde_json.
- `apple_mock` — wiremock, rsa (+rand_core), jsonwebtoken, base64, serde_json, std.
- `sync_probe` — reqwest (stream), tokio (rt/sync/time/macros), tokio-stream,
  uuid, serde_json + the `ApiUrlTarget` seam.

## The two seams
1. **`crate::ApiUrlTarget`** (new, in harness lib.rs, dep-free, always compiled).
   `fn api_url(&self, path: &str) -> String`. `sync_probe::open` is generic over
   it; ziee's `TestServer` shim impls it (inherent `api_url` satisfies it). This
   is the ONLY app→fixture behavioural seam.
2. **`fixture_p8_path()` / `CARGO_MANIFEST_DIR`** (apple_mock, unchanged source).
   Resolves `<harness>/tests/fixtures/apple_test_key.p8`. The `.p8` moved WITH the
   fixture, so the path stays valid + self-consistent for any consuming app. This
   is a fixture-local file reference, NOT an app coupling.

## Call-site inventory (all compile UNCHANGED via the re-export shims)
- `tests/auth/oauth_test.rs` → `oauth_mock::OAuthMockServer`.
- `tests/auth/ldap_test.rs` → `ldap_mock::…`.
- `tests/auth/apple_test.rs` → `apple_mock::AppleMockServer` + `fixture_p8_path()`.
- `tests/sync/subscribe_test.rs`, `tests/user/sync_emit_test.rs`,
  `tests/llm_provider/sync_emit_test.rs` → `sync_probe::SyncProbe::open(&server, …)`.

## STAY (not in scope — domain fixtures, left ziee-side)
`stub_chat.rs`, `stub_engine.rs`, `oai_capture_stub.rs` (LLM/engine, AI-domain),
`chat_stream_probe.rs` (chat-specific). `harness_inner.rs` already SDK-backed.

## Cross-platform / environmental notes
- testcontainers (`oauth`/`ldap`) need docker; available on this box (no sandbox
  needed). apple/sync need no docker. No `#[cfg(target_os)]` gating in any moved
  fixture — all four are OS-neutral.
- `.env.test` is absent in this worktree; none of these 4 gates need its keys
  (HF/ANTHROPIC), so the runs are unaffected.
