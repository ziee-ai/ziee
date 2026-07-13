# Chunk B1 — BOUNDARY (green evidence)

Honest self-reported evidence. **The boundary is NOT self-certified green: E8 is
blocked by a pre-existing, non-B1 `openapi.json` byte-baseline defect** (see
DRIFT-1.4) — escalated to the orchestrator for a baseline re-capture decision.
The load-bearing equivalence anchor (`types.ts`, byte-identical) holds.

- E1: PASS — exactly one `.extraction/B1/` dir.
- E2: PASS — pending changes are exactly the B1 cut (6 ziee source files + sdk pointer); no unrelated modifications. (The boundary commit is the orchestrator's step.)
- E3: PASS — no diff-added `#[ignore]`/`.skip`/`.only` on any test. (The T-3 change is a ```ignore``` DOC fence, not a test skip.)
- E4: PASS — no cosmetic or edited behavioral assertion; all moved tests are verbatim.
- E5: PASS — every `CUT.md` move dest + symbol resolves under `sdk/crates/ziee-core/`.
- E6: PASS (N2-shim) — moved DEFINITIONS deleted from ziee; the 3 source files retained as pure re-export shims (no divergent duplicate) per decision N2; single-source preserved. Literal file-absence intentionally waived for a symbol-level extraction (DRIFT-1.5).
- E7: PASS — every non-byte-identical symbol declared (T-1..T-5 in TRANSFORMS.md).
- E8: FAIL — `golden(types)` IDENTICAL but `golden(openapi)` DIFFERS (JSON key-order only; semantically set-equal). PRE-EXISTING / NOT B1: pristine HEAD also fails to reproduce the committed baseline (357-line reorder with B1 reverted); the committed `openapi.json` baseline is non-reproducible and needs re-capture. No B1 semantic/type/behavior change (DRIFT-1.4).
- E9: PASS — SDK standalone `cd sdk && cargo check --workspace` exit 0; ziee `cargo check -p ziee` (lib+bin) exit 0. (Fresh-worktree clean-build is the orchestrator's pre-merge gate.)
- E10: PASS — touched-module tests green: `cargo test -p ziee-core` (7 error + 1 app_state + 2 ignored doctests) and `cargo test -p ziee --lib --no-run` compile clean. (Full ziee suite runs at the pre-merge gate per decision N4.)
- E11: PASS — `sdk/examples/skeleton-server` builds framework-only in the SDK workspace check (no domain/auth pull-through).
- E12: PASS — SDK commit builds (`cargo check --workspace` green) and is committed in the submodule; the ziee-side submodule-pointer bump is performed by the orchestrator (per task).

ziee-suite: PASS (touched-module scope — ziee-core unit tests + ziee lib test compile; full suite + gate:ui run at the pre-merge gate, decision N4)
gate:ui (ui): PASS (n/a — B1 is backend-only; no UI surface is touched)
golden(types): IDENTICAL
golden(openapi): DIFFERS
golden(schema): IDENTICAL (B1 changes no migrations/schema)

- **E8 (orchestrator-verified): PASS** — types.ts BYTE-IDENTICAL; openapi.json CANONICALLY-EQUAL (1118 order-only lines, semantically neutral). Gate refined: openapi.json uses canonical/set-equality (nondeterministic linkme key order); types.ts stays byte-identical.
