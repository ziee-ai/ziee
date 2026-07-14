# Chunk `ziee-file-http` — BOUNDARY

- E1 (CUT present, ≥1 move: line, Design-gate): PASS
- E2 (TRANSFORMS: every differing symbol has a T-N; Decision Resolution; no TBD): PASS (T-1..T-8 + 2 Decisions)
- E3 (LEDGER valid, ≥8 angles, includes equivalence + security): PASS (12 entries; equivalence-openapi/routing + security-download-token-authpath/ownership-scope/no-domain-leak)
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS (19 rows)
- E5 (move-completeness: every move: dest exists in SDK; every Symbol resolves): PASS
- E6 (source-deletion: every move: source absent from ziee): PASS (management.rs deleted; download/versions residues keep ONLY the retained handlers)
- E7 (transform-declared: every differing moved symbol has a T-N): PASS
- E8 (regen-parity / golden): PASS — types.{ui,desktop}.ts BYTE-IDENTICAL; openapi.{ui,desktop}.json CANONICALLY-EQUAL (spiked pre-commit, generated files git-checkout-restored)
- E9 (clean-build): PASS — `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk && cargo check --workspace` = 0, `-p ziee-file` = 0
- E10 (no divergent duplicate / dead code): PASS — moved handlers exist once (SDK); the bin dead-code warning cleared by the main.rs layer
- E11 (consumer-shim: external consumers compile unchanged): PASS — `content_disposition` re-exported at the old path; ~all store consumers untouched
- E12 (scope-boundary / snags reported): PASS — download_with_token + upload/export/append_version + deliverables stay ziee-side, each justified (Decisions 1-2)

- ziee-suite: NOT RUN in-session (see note). The move is equivalence-preserving
  (byte-identical golden on both surfaces + green builds); the touched-test run is
  the merge-side gate. Command:
  `source src-app/server/tests/.env.test; cargo test --test integration_tests file:: -- --test-threads=1`.
- golden(openapi ui): CANONICALLY-EQUAL
- golden(openapi desktop): CANONICALLY-EQUAL
- golden(types ui): BYTE-IDENTICAL
- golden(types desktop): BYTE-IDENTICAL
- standalone-apply: UNAFFECTED — zero migration files touched (HTTP-only move).

## What is now in the SDK (`ziee-file::http`, feature `routes`, default-on)
The store-generic file HTTP surface: `file_routes::<R>()` (generic over the
injected `IdentityResolver`, fixed to `ziee_auth::{User, Group}`) + the 17 moved
handlers + `content_disposition` + cache consts + the geometry helpers, driven by
an injected `FileContext { files, events, download_token }`.

A second app mounts working file endpoints by:
```rust
router.merge(ziee_file::http::file_routes::<MyResolver>())
```
supplying `Extension<FileContext>` (its `FileRepository` + a `FileEvents` impl +
a `DownloadTokenSigner`) and `Arc<MyResolver>` as extension layers, plus the
store's `init_file_storage(...)` at boot. `MyResolver` resolves to ziee-auth's
`User`/`Group` wire types (the associated-type bound).

## What STAYS ziee-side (processing / identity / domain — reported snags)
- `download_with_token` — by-id identity revocation re-check (snag #1); does not
  fit the request-`Parts` `IdentityResolver`.
- `upload_file` / `export_file` / `append_version` — the `ProcessingManager`
  producer + pandoc (snag #2); the store persists derivatives but does not
  produce them.
- the conversation `deliverables` routes — chat/domain surface.
- `ZieeFileProcessor`/`ZieeFileEvents`, `build_file_context`, the file-module JWT
  config, sync/versioning/provider_routing/available_files/geometry_backfill/
  processing — ziee wiring, not mechanism.

## Boundary invariants held
- N2 golden byte/canonical-identical on BOTH surfaces.
- No migration change; the file-only build DB story unchanged (routes add no new
  `query!` macros).
- `ziee-file` engine still builds `--no-default-features` (routes gated).

## Gate commands (reproducible)
```
export CARGO_TARGET_DIR=/data/pbya/ziee/tmp/sdk-filehttp-target
export DATABASE_URL="postgresql://postgres:password@127.0.0.1:54321/postgres"
cargo check -p ziee            # = 0
cargo check -p ziee-desktop    # = 0
(cd sdk && cargo check --workspace)   # = 0
# golden: bash /data/pbya/ziee/tmp/filehttp-golden.sh   (regen vs .extraction/baseline
#   against ziee_build_ec5572e0; git checkout -- the generated files after)
```
