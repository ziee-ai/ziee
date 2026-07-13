# `extraction-check.mjs` — Gate Spec (buildable)

The deterministic, non-self-certifiable validator that gates the SDK extraction, analogous to
`.claude/lifecycle/lifecycle-check.mjs`. This is the **spec** an implementer builds from; no code
here. Pairs with `SDK_EXTRACTION_PLAN.md` §10 (gate philosophy) and §2 (migration model).

**Design stance (inherited from lifecycle-check):** the machine re-runs the load-bearing checks
itself (regen, build, suite) and reads convergence counts off artifacts — an agent cannot
assert-pass a structural gate. Human-judgment quality (did the audit find the right things) is the
behavioral **P1** rule, not machine-enforced.

---

## 1. Invocation

```
extraction-check.mjs --chunk <id> [--phase P]   # gate one chunk (or one phase of it)
extraction-check.mjs --all                        # gate every chunk in .extraction/, contiguity-checked
extraction-check.mjs --baseline snapshot          # (re)capture the pre-extraction golden baseline
```
- `--chunk <id> --phase P` runs phase P's checks; exits 0 iff `present && gaps.length===0`, printing
  "chunk <id> phase P green — proceed to phase P+1". A PENDING artifact exits 1.
- `--chunk <id>` (no `--phase`) runs C-1..C-5 in order, enforces phase contiguity, exits 0 only if
  all green (the chunk boundary).
- `--all` runs every `.extraction/<id>/` dir in sequence order (see §2 ORDER file), enforces that no
  later chunk is green while an earlier one is PENDING, reports `highest/<N>`. The **pre-push hook**
  runs `--all` on the extraction branch.

Exit codes: 0 green, 1 gaps/pending, 2 malformed artifacts/usage.

---

## 2. Artifacts (per chunk, under `.extraction/<id>/`)

`.extraction/ORDER` (repo-root) lists chunk ids in sequence (`chunk0, B1, B1b, B2, SKELETON, B3, …`).
Each chunk dir holds:

| File | Phase | Required line grammar (regex-parseable) |
|---|---|---|
| `CUT.md` | C-1 | `## Files` then `- move: \`<path>\` → \`<sdk-path>\``; `## Symbols` then `- symbol: \`<name>\` (<file>)`; `## Design-gate` prose |
| `TRANSFORMS.md` | C-1 | `- **T-N** \`<symbol>\`: <what changed> — **why:** <rationale>` ; a `## Decision` block with `**Resolution:**`; **zero** `TBD/TODO/ASK/???` |
| `DRIFT-N.md` | C-2 | `- **DRIFT-N.k** — verdict: manifest-fix\|move-fix\|none\|resolved`; ends `**Unresolved drifts:** <int>` |
| `LEDGER.jsonl` | C-3 | one JSON/line: `{angle,file,line,severity,finding,status}` |
| `AUDIT_COVERAGE.tsv` | C-3 | header `file⇥start⇥end⇥angles`; one row per reviewed diff hunk |
| `FIX_ROUND-N.md` | C-4 | ends `**New confirmed findings:** <int>` |
| `TESTS-MOVED.md` | C-5 | `- **T-<id>** [ported→sdk\|stays→ziee] file: \`<path>\` covers: <moved-symbol>` |
| `BOUNDARY.md` | C-5 | `- E1: PASS` … `- E12: PASS` (+ `EA` for BA); `ziee-suite: PASS`; `gate:ui (<ws>): PASS`; `golden(openapi\|types\|schema): IDENTICAL` |

---

## 3. Phase gates (C-1..C-5) + the E/EA check catalog

### C-1 plan
- **G-CUT** CUT.md present, ≥1 `move:` line, `## Design-gate` present.
- **G-XFORM** every non-byte-identical symbol has a `T-N` with a `why:`; `## Decision` has `**Resolution:**`; **zero forbidden markers** (`TBD/TODO/ASK/???`).

### C-2 move + drift (convergence)
- **E5 move-completeness** every `move:` dest exists in the SDK tree; every `## Symbols` entry resolves in the SDK crate. *(deterministic — fs + `git grep`)*
- **E6 source-deletion** every `move:` source path is **absent from ziee** (no divergent duplicate). *(deterministic)*
- **E7 transform-declared** for every symbol whose SDK form differs from its pre-move ziee form (diff), a `T-N` exists. *(deterministic — diff the moved symbol vs the baseline blob)*
- **DRIFT-converge** final `DRIFT-N.md` `Unresolved drifts: 0`.
- **E3** no diff-added `#[ignore]`/`.skip`/`.only`/`xit`. **E4** no cosmetic assertion AND no edited *behavioral* assertion on a retained ziee test (import-path-only edits allowed; assertion-body edits fail). *(deterministic — diff regex, reuse lifecycle A3/A4 + the new behavioral-edit detector)*

### C-3 blind audit (coverage)
- **E-audit-angles** `LEDGER.jsonl` valid; ≥ `ANGLE_MIN` (=8) distinct angles; MUST include `equivalence`; MUST include `security` when the chunk touches `auth`/`permissions`/`control_mcp`/`identity`.
- **E-audit-coverage** parse `git diff <prev-boundary>...HEAD --unified=0` (excludes `.extraction/`, `**/openapi.json`, `**/api-client/types.ts`); every hunk reconciled against `AUDIT_COVERAGE.tsv` with ≥3 angles.

### C-4 fix (convergence)
- **FIX-converge** final `FIX_ROUND-N.md` `New confirmed findings: 0`.

### C-5 boundary (equivalence + green — the master gates)
- **E8 golden equivalence** (deterministic, validator RE-RUNS): `just openapi-regen` in ziee → `git diff` empty across all 4 generated files vs the `pre-sdk-extraction` baseline; after chunk BA, `pg_dump --schema-only` of a merged-migrator DB byte-identical to the baseline schema snapshot. **See §5 for the genericization caveat.**
- **E9 dual clean-build** (deterministic): `cargo clean && cargo check --tests` for (a) the SDK standalone and (b) ziee-on-pinned-SDK, both from fresh staging worktrees (catches warm-build proc-macro masking).
- **E10 boundary-green** (deterministic, decision N4): per-boundary = **touched-module tests** + golden diffs + dual clean-build; the **full ziee suite + `gate:ui`** run at the **pre-merge gate** (+ nightly), not every boundary.
- **E11 skeleton-agnostic** (deterministic): `sdk/examples/skeleton-server` builds linking only `ziee-core`+`ziee-framework` (+`ziee-control-mcp` only for the control-specific check) — no domain/auth pull-through.
- **E12 submodule-pin** (deterministic): ziee's `sdk` submodule pointer is committed and points at an SDK commit that builds.
- **EA merged-migrator** (chunk BA only, deterministic): the merged Migrator applies on a fresh DB AND a copy of a real ziee DB; identical final schema; no checksum errors; all migration versions timestamped; no edited released migration (checksum-immutability CI guard).
- **E1** exactly one `.extraction/<id>/` dir. **E2** clean working tree (ignores pgvector submodule + `.log`).
- **TESTS-preservation** every `TESTS-MOVED.md` entry PASSes; **A5-shrink-guard** — no covering test id present in an older committed `TESTS-MOVED.md` may be absent now.

---

## 4. Baseline (the equivalence anchor)

`--baseline snapshot` (run once, before Chunk 0) captures into `.extraction/baseline/`:
- `openapi.ui.json`, `types.ui.ts`, `openapi.desktop.json`, `types.desktop.ts` (from `just openapi-regen` on pre-extraction ziee),
- `schema.sql` (`pg_dump --schema-only` of a fully-migrated ziee DB),
- the git tag `pre-sdk-extraction` (the diff base for E7 and the per-symbol transform check).
E8 diffs against these. The baseline is **immutable** for the life of the extraction (a legitimate
generated-output change — see §5 — is a deliberate, reviewed baseline re-capture, logged).

---

## 5. The genericization caveat (must be resolved before build — see decisions)

E8 demands `types.ts` **byte-identical**. But B2 (Config split), B3 (pluggable resolver), B5
(`SyncEntityKind`) genericize types that may appear in the OpenAPI spec → a schema/name change →
`types.ts` changes → E8 *fails by design*. **Two ways to reconcile, pick one:**
- **(a) Equivalence-preserving genericization** — require every refactor to keep the *serialized*
  schema identical (same JSON names/shapes; genericity is internal-only). E8 stays byte-exact. Hard
  discipline, strongest guarantee.
- **(b) Declared-delta baseline** — a chunk may ship a `GOLDEN_DELTA.md` enumerating the exact,
  reviewed spec/types changes it causes; E8 asserts the diff equals the declared delta (not empty).
  More flexible, weaker (a delta can hide a real change).

**DECISION N2: (a) equivalence-preserving + re-export shims.** The byte-identical gate stays
**absolute**. A chunk that touches OpenAPI-facing types keeps thin **ziee re-export shims** so
schemars type-idents/paths don't move, and **spikes its openapi diff on a throwaway branch BEFORE
committing** the chunk. A provably-cosmetic delta needs **human sign-off** — no blanket
declared-delta escape. (Chunk BA — which moves `User`/`Group`/`Session` — is the highest-risk case
and MUST be spiked first.)

---

## 6. Cross-repo + reuse

- **Per-chunk base** = the prior chunk's boundary commit (from `.extraction/ORDER`), NOT `origin/main`.
- **Reuse from `merge-gate.mjs`**: E8↔C3 (regen-parity), E9↔C1 (clean-build), EA↔C2 (migration), E5↔P2
  (completeness), plus C5 lifecycle-strip at final merge. The final extraction-branch→main merge runs
  the existing `merge-gate.mjs` unchanged.
- **Enforcement**: per-chunk-boundary gate + a pre-push hook running `--all`. Deterministic/external.

---

## 7. Build order

`extraction-check.mjs` + the baseline snapshot + the `skeleton-server` are **prerequisites of Chunk 0**
(the gate must exist before it can gate). They are Phase-1 tooling tasks (see
`PHASE1_EXECUTION_PLAN.md`).
