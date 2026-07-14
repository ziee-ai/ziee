# Chunk `ziee-test-harness` — FIX round 1

Findings from the blind audit (LEDGER) triaged; all 13 are `info`/verified with
no behavioural regression. Two mechanical issues were caught + fixed DURING
implementation (pre-commit), logged here for the record:

- **FIX-1.1** — let-chain `if let … && …` did not compile under the SDK
  workspace's edition 2021 (server crate is 2024). Rewrote to nested `if let` +
  `if`. Semantically identical; `cargo check -p ziee-test-harness` = 0 after.
- **FIX-1.2** — the desktop test binary `#[path]`-reincludes the shim, which
  now names `ziee_test_harness`; the desktop crate had no dev-dep on it (it had
  relied on `ziee-build-support` being reachable via the old import). Added
  `ziee-test-harness` to `src-app/desktop/tauri/[dev-dependencies]`.

No NEW findings surfaced by the audit that required a code change (the config
YAML, Drop, template-clone, and test_helpers are provably byte-preserved; the
manifest_dir + Variant + before_spawn de-couplings are the intended, declared
transforms).

**New confirmed findings:** 0
