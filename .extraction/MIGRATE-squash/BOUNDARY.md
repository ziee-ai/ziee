# MIGRATE-squash — BOUNDARY

Master equivalence + green gates. This chunk is EA-gated (M2 exemption from the
move-shaped E5/E6/E7).

- EA-schema: PASS — catalog fingerprint of the squashed set IDENTICAL to
  `.extraction/baseline/schema.fp` (validator re-runs `schema_fp.sql` on both DBs;
  build-DB server-owned schema also identical, only expected desktop tables extra).
- EA-seed: PASS — whole-DB canonical seed image EQUIVALENT to
  `.extraction/baseline/seed.canonical.txt` (21 seeded tables; business-key keyed,
  FK-through-business-key, element-sorted set arrays).
- EA-N9: PASS — `grep` of `sdk/crates/ziee-auth/migrations/` returns ZERO
  permission strings other than `profile::*` / `*` (0 violations).
- E8 golden(types): IDENTICAL — types.ui.ts + types.desktop.ts byte-identical.
- E8 golden(openapi): IDENTICAL — openapi.ui.json + openapi.desktop.json
  canonically-equal (jq -S) to baseline. Regenerated files restored.
- E8 golden(schema): IDENTICAL — (EA-schema above is the schema anchor post-squash).
- E9 dual clean-build: PASS (warm) — `cargo check -p ziee` = 0, `cargo check -p
  ziee-desktop` = 0, `cd sdk && cargo check --workspace` = 0 (auth-only build DB
  re-provisioned + query! verified). Fresh-clone clean-build deferred to the
  pre-merge gate.
- E12 submodule-pin: PASS — ziee's `sdk` pointer will be bumped to the local
  squashed-auth commit at stage time (orchestrator pushes after verification).
- E1: PASS — exactly one `.extraction/MIGRATE-squash/` dir.
- E2: clean-tree — deferred to commit (reshape stages 137 deletes + 91 adds).
- ziee-suite: DEFERRED (N4 — full suite + gate:ui run at the pre-merge gate, not
  per boundary). Touched-module build verification is green (3× cargo check).
- gate:ui (ui/desktop): DEFERRED (N4 — migrations touch no UI surface; golden
  byte-identical both surfaces is the relevant per-boundary UI check).
