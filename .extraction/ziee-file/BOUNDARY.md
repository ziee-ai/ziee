# Chunk `ziee-file` — BOUNDARY

- E1 (CUT present, ≥1 move: line, Design-gate): PASS
- E2 (TRANSFORMS: every differing symbol has a T-N; Decision Resolution; no TBD): PASS
- E3 (LEDGER valid, ≥8 angles, includes equivalence + security): PASS (13 entries, 10 distinct angles)
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS (29 rows)
- E5 (move-completeness: every move: dest exists in SDK; every Symbol resolves): PASS
- E6 (source-deletion: every move: source absent from ziee): PASS
- E7 (transform-declared: every differing moved symbol has a T-N): PASS
- E8 (regen-parity / golden): PASS — types.{ui,desktop}.ts BYTE-IDENTICAL; openapi.{ui,desktop}.json CANONICALLY-EQUAL
- E9 (clean-build): PASS — `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk && cargo check --workspace` = 0
- E10 (no divergent duplicate / dead code): PASS
- E11 (consumer-shim: ~59 store consumers compile unchanged): PASS
- E12 (domain-agnosticism / N9: store names no domain table): PASS
- EA-schema: INTENDED DELTA ONLY — removed `files.workflow_run_id` (col+FK+index), added `file_workflow_runs` (table+2FK+PK+index); relationship preserved via join (RESOLVED). Composition proven by the golden ziee build.

- ziee-suite: PASS (touched) — the join-table run-linking behavior is green:
  `workflow::run_history_and_delete` (all), `workflow::tool_step::tool_step_resource_link_is_saved_{false,true}`,
  `mcp::resource_link_test::*`, `mcp::sync_emit_test::tool_call_resource_link_persists_file_and_emits_sync` all OK
  after re-homing 6 test queries from `files.workflow_run_id` → the `file_workflow_runs` join.
  Pre-existing/env failures (NOT this chunk): real-LLM tests (placeholder API keys in .env.test),
  `file_attachments_test` (needs a chat provider), `tool_step_prior_step_whole_value_ref_is_a_real_array`
  (unrelated `/tmp` workflow file_io: missing `consume.txt`), and
  `test_file_content_responses_have_private_bounded_cache_control` (untouched `management.rs` preview
  handler returns `private, no-cache` — a base test/code drift, not this extraction).
- golden(openapi): IDENTICAL
- golden(types): IDENTICAL
- golden(schema): EQUIVALENT (intended workflow_run_id→side-table delta; not byte-identical by design, per RESOLVED)

## Gate commands (reproducible)
```
export CARGO_TARGET_DIR=/data/pbya/ziee/tmp/sdk-file-target
export DATABASE_URL="postgresql://postgres:password@127.0.0.1:54321/postgres"
cargo check -p ziee            # = 0
cargo check -p ziee-desktop    # = 0
(cd sdk && cargo check --workspace)   # = 0
# golden: regen vs .extraction/baseline against ziee_build_12381b49 (merged DB)
# schema fp: psql -X -q -t -A -F $'\t' -f .extraction/tools/schema_fp.sql
```
