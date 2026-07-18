# MIGRATE-squash — Phase 1 gate-tooling proof (B1 validation)

The equivalence gate was built and **proven before any squash**, per the chunk
contract. Tooling (committed under `.extraction/tools/`):

- `schema_fp.sql` — EA-schema catalog-derived structural fingerprint
  (`psql -X -t -A -F $'\t' -f`). Sections: extensions, columns
  `{table.col → (type, null, gen, generation_expr)}`, constraints via
  `pg_get_constraintdef` (auto-name excluded from the def), non-constraint
  indexes via name-stripped `pg_get_indexdef` (opclass/predicate/method kept),
  enums, sequences, user functions (extension-owned excluded), triggers. Every
  section globally `ORDER BY`'d → order/name/attnum-invariant.
- `seed_compare.py` — EA-seed. Live two-DB compare **and** `--emit` canonical
  image. Drops volatile cols (`created_at`/`updated_at`/`*_at`/`*_encrypted` +
  gen_random_uuid surrogate PKs), resolves FK cols **through the referenced
  row's business key**, **element-sorts** `TEXT[]` set columns, keys rows by
  natural business key (`groups.name`, …) else multiset-compares.
- `apply_migrations.sh` — drop+recreate a scratch DB and apply a migration set
  version-sorted by basename, exactly as `build.rs` composes it.

## The B1 validation results (all on the `:54321` pgvector-pg18 cluster)

| Check | Input | Result |
|---|---|---|
| **Determinism (schema)** | numeric merged set applied to two fresh DBs (`ea_baseline_a`, `ea_baseline_b`) | `schema.fp` **IDENTICAL** |
| **Determinism (seed)** | same two DBs | seed_compare **EQUIVALENT** (21 tables); `--emit` byte-identical |
| **Sensitivity — drop column** | `ALTER assistants DROP COLUMN description` on a squash clone | fingerprint **DETECTED** |
| **Sensitivity — generated expr** | `bibliography_entries.content_tsv` regconfig `english→simple` | fingerprint **DETECTED** (COL + dependent IDX lines) |
| **False-positive — perm reorder** | random-shuffle `groups.permissions` (`Users`) | seed_compare **did NOT false-fail** (element-sort) |
| **Sensitivity — perm change** | append `evil::perm` to `Users` | seed_compare **DETECTED** |

The identical-input determinism case is byte-stable for BOTH anchors, so the
tooling is safe to gate a squash (no non-determinism to mask a real diff).

## One reconstruction subtlety found + fixed (not a fingerprint tweak)

pg_dump (schema source of truth) round-trips VARCHAR `CHECK (col IN (...))`
**non-idempotently** on PG18: its expanded `((col)::text = ANY ((ARRAY[
'x'::character varying, …])::text[]))` re-normalizes on re-apply to an
element-cast variant whose `pg_get_constraintdef` text differs from baseline
(27 constraints). Proven fixed-point: the **original** `col IN (...)` form
stores the *identical* catalog form as baseline. The reconstruction therefore
rewrites those 27 expanded CHECKs back to `IN (...)` (`normalize_checks.py`) —
this reproduces the baseline author's DDL, it does **not** weaken or special-case
the fingerprint. After the rewrite: `schema.fp` squash-vs-baseline **IDENTICAL**.

## Baseline anchors captured (immutable)
- `.extraction/baseline/schema.fp` — EA-schema fingerprint of the numeric-merged DB.
- `.extraction/baseline/seed.canonical.txt` — `seed_compare --emit` canonical image (self-contained; re-runnable text diff, since the numeric migration set is deleted by the squash).
- `.extraction/baseline/seed.sql` — raw `pg_dump --data-only --inserts` reference image.
