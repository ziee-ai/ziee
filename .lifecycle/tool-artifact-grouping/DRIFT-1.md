# DRIFT-1 — implementation vs plan

Audit of the implemented diff against PLAN.md / TESTS.md.

- **DRIFT-1.1** — verdict: impl-wins — PLAN.md ITEM-4 originally specified a single `deriveGroupOpen({ hasPendingApproval, hasRunning, hasArtifact, userOpen })`. The implementation splits it into `shouldAutoOpen({ hasRunning, hasArtifact })` (the default-open latch) + `deriveGroupOpen({ hasPendingApproval, userOpen })` (the render decision). A single OR of all four would force the group open *continuously* while a tool is running, blocking the user from collapsing it — contradicting the approved "running/artifact default-open but stay user-collapsible" policy. Only pending-approval force-opens. Resolved by amending PLAN.md ITEM-4 + TESTS.md TEST-2 to the two-function shape; re-ran `--phase 1/2/3` green.

- **DRIFT-1.2** — verdict: none — the gallery deep-state (`deep-chat-tool-group` in `chat-deep.ts` + `deepStates.tsx`) is within PLAN.md's "Files to touch" ("plus a gallery state under `src-app/ui/src/dev/gallery/` if one is needed"). It exercises the new group-with-artifact render under `gate:ui`/`runtime-health` and satisfies `check:state-matrix`. No plan divergence.

- **DRIFT-1.3** — verdict: resolved — regenerating `stateMatrix.generated.ts` + `STATE_MATRIX.md` (required by `check:state-matrix` after touching `ChatMessage.tsx`/`extension.tsx` and adding the gallery state) also picked up PRE-EXISTING base drift: the committed matrix was already stale at `origin/khoi` (e.g. `MessageList.tsx` — source unchanged by this branch — has shifted signal line numbers). These are mechanically-generated files (like the `openapi.json`/`api-client/types.ts` the coverage law excludes); the reviewed artifact is the SOURCE, not the generated output. Regen is the gate's own prescribed fix (`npm run gen:state-matrix`), committed here; no source behavior rides in it.

**Unresolved drifts:** 0
