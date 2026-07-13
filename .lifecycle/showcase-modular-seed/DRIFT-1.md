# DRIFT-1 — implementation vs plan (Phase 5)

Reconciling the shipped implementation against PLAN.md after all items (1-18) were
implemented + both workspaces' `npm run check` went green.

- **DRIFT-1.1** — verdict: impl-wins — ITEM-14 planned to allow-list "the 5 INFRA-ONLY
  modules". Reality: the `hasUserSurface` heuristic AUTO-EXCLUDES the 4 truly
  surfaceless infra modules (config-client/layouts/router/dev-gallery — no route,
  no user slot), and `settings` owns a real seeded `settings` shadow → it gets a
  `gallery.tsx`, not an allow-list. So the UI allow-list is EMPTY. The desktop
  allow-list has exactly ONE entry (`memory`) — a desktop override whose
  `gallery.tsx` would raw-shadow ui's and fail the override-registry gate, so it is
  deliberately crawl-covered. Additionally the 3 previously OVERLAY-allow-listed
  drawers (Knowledge/Scheduled-task/Upload) got WIRED, so their overlay-allowlist
  excuses were removed (the GC check enforced this). Net: the allow-list mechanism
  exists + is exercised (1 desktop entry), just not the 5 the plan guessed. PLAN
  understanding amended; no new ITEM.

- **DRIFT-1.2** — verdict: resolved — the shared helpers were split for the Node test
  loader (it transpiles `.ts` but not `.tsx`): `support/hold.ts` (JSX-free timing
  helpers, node-testable) + `support/lazy.tsx` (lazy-render helpers); and the pure
  registry logic moved to `support/registry-core.ts` (no `import.meta.glob`, so
  TEST-6/7 run in plain Node). The barrel re-exports both. Impl detail of ITEM-1/2;
  reconciled.

- **DRIFT-1.3** — verdict: impl-wins — the plan anticipated ONLY the overlay-registry
  gate coupling (ITEM-18). In fact THREE more existing gates walk `modules/**/*.tsx`
  and had to learn to exclude the new `gallery.tsx` (authoring metadata, not a
  surface): `gen-gallery-coverage`, `gen-state-matrix`, `gen-testid-registry` — in
  BOTH workspaces (desktop has its own copies of coverage + state-matrix). Necessary
  integration edits; folds under ITEM-4/15. The plan under-counted the gate-coupling
  surface — a generalizable lesson (any gate that walks module source must skip
  `gallery.tsx`).

- **DRIFT-1.4** — verdict: impl-wins — ITEM-15 said "desktop aggregators glob shared +
  desktop-only". The cleaner realized design: desktop `module-seed.ts` REUSES the web
  workspace's `MODULE_CASSETTE`/`OVERLAY_ENTRIES` (imported via the `@/` override
  fallback — the web registry's eager glob is anchored to `ui/src/modules`, so it
  returns the shared seed regardless of which build imports it) PLUS a desktop-local
  glob for desktop-only modules. This is type-safe (verified: desktop `tsc` clean)
  and avoids a raw cross-workspace glob in every aggregator. Refinement of ITEM-15.

- **DRIFT-1.5** — verdict: impl-wins — a gate-accuracy bug surfaced during impl:
  `hasUserSurface` matched a COMMENTED-OUT route (`window`'s `//   path: …`),
  false-flagging `window` as surface-bearing. Fixed by stripping comments first
  (mirrors `gen-override-registry`'s `stripComments`); added a unit test. Not in the
  plan; a correctness improvement to ITEM-7.

- **DRIFT-1.6** — verdict: resolved — TEST-9 used a JSON `assert { type: 'json' }`
  import (rejected by the TS target) → switched to `fs.readFileSync` of the committed
  baseline. Spec-mechanics fix.

- **DRIFT-1.7** — verdict: resolved — TEST-12's file-detail surface needs the URL to
  pin `fileId` (the gallery's isolated-detail convention) + a benign-`403` console
  filter (a non-`/api` asset load; matches the runtime-health gate's filtering). Spec
  correctness, not a seed defect.

**Unresolved drifts:** 0
