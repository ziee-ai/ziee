# AUDIT — workflow-kind-agent (Phase 6, blind multi-angle)

Four blind auditors (diff-only, no design context) reviewed the full `feat/agent-core...HEAD` diff
(37 files, 3.68k insertions) across ≥3 angles per hunk (see AUDIT_COVERAGE.tsv, 80 hunk rows).
**22 confirmed findings: 3 HIGH, 9 MEDIUM, 10 LOW** (LEDGER.jsonl). The 3 HIGHs were re-verified
against the actual code before confirming. No finding is a fundamental plan flaw — all are fixable in
Phase 7, so no ITEM is BLOCKED.

## Angle coverage
- **Backend** (correctness/security/concurrency/api-contract/resource-cleanup/maintainability): 1 HIGH
  (destroy-bundle-before-validate on PUT /definition), 2 MED (single-file-only bundle repack; create
  name-collision overwrite+orphan), 2 LOW (activity trim by ordinal not seq; validate-def /tmp probe).
  Clean: AtomicU64 seq, distinct track ids, JSONB append atomicity, ownership 403/404, least-privilege
  perms, aide docs.
- **Builder correctness/hooks/store**: 2 HIGH (llm & llm_map `tools` picker → validate-def E6 blocking
  error → Save disabled), 2 MED (delete-while-open save PUTs to a 404 instead of recreate; tool-args
  coerced to strings), 1 LOW (stale local JSON buffer on cross-device refetch).
- **Builder UI/quality/gallery**: 3 MED (unwired LabeledControl `htmlFor` → nameless InputNumbers;
  elicit raw-JSON no aria-label; `as unknown as` cast + misleading "no unsafe cast" comment → StepDef
  drift), 5 LOW (hand-rolled header vs kit SectionHeader; `!important`; useState-as-ref; index keys;
  magic `mt-6`).
- **Run timeline**: 2 MED (uncapped agentActivity array + O(n²) merge; 390px title overflow on a long
  unbroken URL/DOI token), 2 LOW (sort-per-render; empty-stepId phantom step).

## Per-item verdicts
- **ITEM-1** (GET /definition) — verdict: PASS — owner-scoped, 404 on cross-user; no finding.
- **ITEM-2** (POST /workflows) — verdict: CONCERN — name-collision silently overwrites + orphans runs (MED); add a 409/unique-name in Phase 7.
- **ITEM-3** (PUT /definition) — verdict: CONCERN — HIGH: extract wipes the bundle before validate (corrupts on invalid def); MED: single-file repack loses multi-file assets. Fix = validate-first + overlay yaml onto the existing dir (no wipe).
- **ITEM-4** (validate-def) — verdict: CONCERN — LOW: shared /tmp bundle_root path-probe; use a unique/empty root.
- **ITEM-5** (agent-activity stream) — verdict: CONCERN — LOW: trim by insertion ordinal can drop a higher seq at the 500 cap; retain the highest-seq window.
- **ITEM-6** (builder store + FE types) — verdict: CONCERN — MED: delete→404 save (branch on deletedExternally → create); MED: `as unknown as` cast masks StepDef drift + misleading comment.
- **ITEM-7** (builder surface) — verdict: CONCERN — LOW: use kit SectionHeader; drop `!important`; fix `mt-6` alignment.
- **ITEM-8** (per-kind forms) — verdict: CONCERN — 2 HIGH (remove the llm/llm_map `tools` picker — backend rejects it); MED tool-args string coercion; MED elicit aria-label; LOW useState-ref/index-key/stale-buffer.
- **ITEM-9** (agent friendly form) — verdict: CONCERN — MED: the shared LabeledControl leaves the advanced exact-max_steps InputNumber nameless; fix the label association.
- **ITEM-10** (ref-insert) — verdict: PASS — no finding.
- **ITEM-11** (activity descriptors) — verdict: PASS — pure map, no finding.
- **ITEM-12** (timeline renderer) — verdict: CONCERN — MED 390px title overflow (break-words/ellipsis); LOW sort-per-render (useMemo).
- **ITEM-13** (run-store agent_activity) — verdict: CONCERN — MED uncapped O(n²) merge (cap the store array + O(1) dedupe); LOW empty-stepId guard.
- **ITEM-14** (openapi regen) — verdict: CONCERN — the flatten-lossy StepDef generator is the root of the ITEM-6/8 cast workaround; parity tests pass + wire is correct, but log/decide the schema-gen fix.
- **ITEM-15** (desktop parity) — verdict: PASS — shared ui/ modules; desktop api-client regenerated; no finding.
- **ITEM-16** (gallery) — verdict: PASS — 193/193 gate:ui; the drawer useNavigate regression was already fixed (MemoryRouter wrapper).

## Phase 7 plan (fix-to-convergence)
Fix all 3 HIGH + 9 MEDIUM + the quick LOWs; re-audit the changed hunks to 0 new confirmed findings.
The StepDef-flatten (ITEM-14) will be addressed by making the generated type non-lossy OR a
compile-time `StepBase` assertion + honest comment (removing the `as unknown as`), plus an integration
round-trip test proving the wire format preserves base fields. Marginal LOWs (activity trim-by-seq,
/tmp probe) fixed or recorded with rationale.
