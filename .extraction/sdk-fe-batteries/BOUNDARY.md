# Chunk sdk-fe-batteries — BOUNDARY (green evidence)

Honest self-reported evidence. This is an ADDITIVE chunk (no move), so the
extraction-specific checks (E5 dest-resolves / E6 source-absent / byte-equivalence
golden) are reframed around the real anchor: **backward-compat** + **no codegen
impact**.

- E1: PASS — exactly one `.extraction/sdk-fe-batteries/` dir; appended to `.extraction/ORDER`.
- E2: PASS — the SDK diff is exactly the three fixes (FE-2 core.ts+index.ts, FE-1 router/* + framework package.json, FE-3 kit.css + README + kit package.json) plus the test-infra resolver + the two smoke tests + the FE-3 proof script; no unrelated modifications.
- E3: PASS — no `#[ignore]`/`.skip`/`.only` added; the smokes assert real behavior.
- E4: PASS — no cosmetic test; the FE-3 proof drives a real Tailwind compile + oxide Scanner and asserts emitted CSS, FE-2 asserts real token dispatch, FE-1 asserts real config DI.
- E5: PASS — every `CUT.md` new file exists and every declared public export resolves (framework package + kit package each `tsc --noEmit` exit 0).
- E6: N/A — additive chunk; nothing moved out of ziee, so there is no source to be absent. ziee's own `ui/src/modules/router` is left untouched (the SDK router is a separate optional package surface).
- E7: PASS — every non-trivial change declared (T-FE2-1/2, T-FE1-1..5, T-FE3-1/2, T-INFRA-1 in TRANSFORMS.md), each with a Decision.
- E8: PASS (reframed) — **NO `types.ts` / openapi impact.** The diff touches only `sdk/packages/{framework,kit}/**`; zero Rust / `emit_ts` / `openapi.json` / `api-client/types.ts` paths. golden(types) is untouched by construction (STOP condition not triggered).
- E9: PASS — `@ziee/framework` + `@ziee/kit` each `tsc --noEmit` exit 0 standalone.
- E10: PASS — new smokes green: FE-2 4/4, FE-1 3/3, FE-3 proof PASS. Backward-compat: ziee `ui/` tsc 0, `desktop/ui/` tsc 0, ziee `ui/` `vite build` 0.
- E11: PASS — tree-shakeable opt-in: router absent from the main barrel; react-router-dom optional-peer.
- E12: PASS — `npm install` at the worktree root clean; SDK commit built + committed in the submodule; the ziee-side submodule-pointer bump + `.extraction/` staging is this chunk's ziee-side (per task, staged not pushed).

ziee-suite: PASS (backward-compat scope — both UI workspaces tsc clean + ziee ui vite build green; this is a frontend-package additive change, no backend/integration surface touched)
gate:ui (ui): N/A-reframed — no ziee UI surface changed (ziee doesn't adopt the router and keeps its own index.css); the backward-compat proof is the ziee-ui `vite build` green + both-workspace tsc clean.
golden(types): UNTOUCHED (no codegen in diff)
golden(openapi): UNTOUCHED (no codegen in diff)
golden(schema): UNTOUCHED (no migrations in diff)

Backward-compat is GREEN; no codegen impact; three fixes shipped with functional proofs. Self-reported PASS (orchestrator re-verifies tsc on both workspaces + merges).
