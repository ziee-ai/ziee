# MIGRATE-squash — TESTS-MOVED

This chunk moves **no test code** — it is a migration-chain reshape. The
migrations are exercised by the existing suite unchanged (every `query!` verifies
against the merged schema at build time; harness/integration tests migrate a
fresh DB via the same composed set). No test file is ported or retired.

The equivalence gate itself is the "test" for this chunk and is machine-enforced
by EA-schema + EA-seed + N9 (BOUNDARY.md), not by a moved unit/integration test.

- **T-none** [stays→ziee] file: `src-app/server/tests/common/harness_inner.rs` covers: merged-migrator applies on the per-test template DB (unchanged; now composes the squashed set)
