# Chunk `sdk-gallery` — FIX round 1

The blind audit (LEDGER, 12 angles incl. equivalence + security) surfaced no
behavioural regression. Four mechanical issues were caught + fixed DURING
implementation (pre-commit), logged for the record:

- **FIX-1.1** — package tsc followed the `@ziee/kit` CSS side-effect import
  (`overlayscrollbars.css`) and `mockApi`'s `import.meta.env.DEV`, erroring under
  the package's own tsconfig. Added `src/env.d.ts` (`declare module '*.css'` +
  `ImportMeta.env.DEV`), mirroring how the app tsconfigs satisfy these. Package
  tsc = 0 after.

- **FIX-1.2** — `GalleryConfig.ThemeProvider` was typed `ComponentType<{children?}>`
  but ziee's real `ThemeProvider` requires `children` — `ThemeProviderProps` not
  assignable. Tightened to `ComponentType<{ children: ReactNode }>`. ui tsc = 0
  after. The other DI component/hook fields (ErrorBoundary/Loading/
  LazyComponentRenderer/useRoutesStore) are intentionally loosely typed
  (`ComponentType<any>` / `(sel:(s:any)=>any)=>any`) — they are runtime seams the
  framework only renders/calls, so the app's concrete components assign without
  variance battles.

- **FIX-1.3** — desktop broke on the deleted `@/dev/gallery/support/registry-core`
  (imported by desktop's `module-seed.ts` cross-workspace bridge). Restored the
  path as a thin re-export shim to `@ziee/gallery`'s pure registry fns. Desktop
  tsc = 0; desktop's local framework copies compile + behave identically. (The
  full desktop mountGallery rewire is a declared follow-up, not a regression.)

- **FIX-1.4** — the config-driven `runtime-health.mjs` copy's overwrite of
  ziee's committed `RUNTIME_FINDINGS.md` during the verification run was reverted
  via `git checkout`; `RUNTIME_FINDINGS.jsonl` is gitignored (generated) and not
  part of the diff. Verified clean afterward.

No NEW findings surfaced by the audit that required a code change: the mock
engine, the four frames, the SSE/state-mode logic, the interaction engine, and
the gate in-page audit are provably byte-preserved; the generic-type / DI / glob-
injection / config de-couplings are the intended, declared transforms (T-1..T-10).

**New confirmed findings:** 0
