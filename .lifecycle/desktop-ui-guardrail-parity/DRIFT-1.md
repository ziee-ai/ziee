# DRIFT-1 — implementation vs plan

Reconciliation after implementing the guardrail backfill + running the audit.

- **DRIFT-1.1** — verdict: impl-wins — PLAN ITEM-2/ITEM-3 said "wire the geometry/
  affordance gate into desktop `gate:ui.mjs`". Reading the WEB `gate-ui.mjs` showed
  it does NOT run geometry/affordance inline either — they are standalone npm
  scripts (`gallery:geometry:gate`, `gallery:affordance`, `detector:acceptance`).
  True parity = standalone scripts, not gate-ui wiring. PLAN ITEM-2/3 + Files-to-
  touch amended (gate-ui.mjs untouched); TESTS TEST-2/3 amended. Resolved.

- **DRIFT-1.2** — verdict: impl-wins — PLAN ITEM-4 assumed a faithful copy of
  `detector-acceptance.mjs`. It requires a `seeded-defect-repro` surface
  (`DefectRepro.tsx`, 488 lines of CHAT-specific geometry repros) the desktop
  page-focused gallery does not have. Faithful copy would import an irrelevant
  apparatus. Adapted the desktop harness to run the LINT detector cases against
  the copied fixtures + a byte-identity drift-guard on the geometry detector (see
  DEC-2, amended). `detector-acceptance.mjs` PASSES. Resolved.

- **DRIFT-1.3** — verdict: impl-wins — PLAN ITEM-7 assumed the desktop chrome
  (window title-bar, file-dialog, updater dialog, app-layout) was gallery-storyable.
  Verification: `window`/`file-dialog`/`desktop-base` ship NO `.tsx` (module.tsx +
  store.ts only — native-OS window controls + native Tauri dialogs), so there is
  ZERO DOM to render or audit; `updater` `UpdateBanner` renders only under a
  Tauri-updater "available" state unreachable in a browser gallery and is built
  from already-audited shared kit; `layouts` app-layout is full router+slot+module-
  graph coupled and the gallery renders pages shell-free BY DESIGN. Building
  Tauri-mock stories would be fragile FALSE coverage that destabilizes the gate
  (net-negative). ITEM-7 amended to an auditability DETERMINATION (recorded in
  `DESKTOP_UI_FINDINGS.md`); the desktop-only ROUTE surfaces that DO render are
  audited. Resolved.

- **DRIFT-1.4** — verdict: impl-wins — the audit RUN surfaced a real desktop-only
  defect NOT in the original items: the desktop `vite-plugin-testid-unique.js`
  diverged from the web plugin (missing the `DefectRepro.tsx` `TESTID_EXEMPT`) and
  ABORTED the desktop gallery build at `buildStart` on a duplicate testid in the
  scanned shared tree. This is exactly the class of guardrail-parity defect this
  feature targets. Fixed (F1); PLAN ITEM-9/10 + Files-to-touch amended to name it.
  Resolved.

- **DRIFT-1.5** — verdict: impl-wins — PLAN ITEM-9/10/11 assumed a backlog of
  desktop-only geometry/contrast/a11y/affordance HIGH findings to fix. The audit
  found 0 gating HIGH on every gallery-auditable desktop-only surface; the only
  real defect is F1 (DRIFT-1.4), and the residual MEDIUM/LOW findings (G5 tap-
  target, I1 kit-Switch hit-test, error-state console logs, off-grid spacing) are
  triaged benign shared-kit/intentional per DEC-7. ITEM-9/10/11 wording amended to
  record this outcome. The e2e verification (TEST-8..12) was re-scoped from
  "new chrome stories" to the auditable route surfaces + the geometry/affordance
  gate scripts, consolidated into one runnable spec `gallery-desktop-runtime.spec.ts`.
  Resolved.

**Unresolved drifts:** 0
