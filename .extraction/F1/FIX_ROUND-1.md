# Chunk F1 — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 14 findings across 14 angles incl.
`equivalence`, `domain-agnostic`, `coupling-severed`, `self-contained-deps`,
`import-rewrite-correctness`, `module-resolution`, `dep-completeness`, `gate-tsc`,
`single-source`, `submodule-wiring`, `design-tokens`, `api-surface`,
`build-hygiene`, `tooling-followup`) reconciled against every diff hunk
(`AUDIT_COVERAGE.tsv` — 10 hunks, ≥3 angles each). Two items were handled DURING
implementation (the drift-convergence loop), not deferred:

- **module-resolution** (deep subpath imports 2307): the first `tsc --noEmit` in
  `ui/` failed on ~25 deep imports (`@ziee/kit/kit/theme`, `@ziee/kit/shadcn/field`,
  `@ziee/kit/testIds.generated`, …). `--traceResolution` showed the package
  `exports` wildcard `"./*": "./src/*"` mapped them to an EXTENSIONLESS target,
  which tsc does not extension-substitute — so subpaths didn't resolve while the
  barrel did. Fixed by adding `@ziee/kit` + `@ziee/kit/*` `paths` to BOTH
  `ui/tsconfig.json` and `desktop/ui/tsconfig.json` (the identical pattern the repo
  already uses for `@ziee/ui-core`); the package `exports` stays for runtime
  bundlers. Re-verified: all three `tsc --noEmit` exit 0.

- **build-hygiene** (CSS side-effect import 2882): the kit's standalone tsc failed
  on `kit/scroll-area.tsx`'s `import 'overlayscrollbars/overlayscrollbars.css'`
  (ziee gets the ambient decl from `vite/client`). Fixed with a self-contained
  `src/css.d.ts` (`declare module '*.css'`) + `allowImportingTsExtensions` in the
  kit tsconfig (for `table-view-core.test.ts`'s `.ts`-extension import). Kit tsc
  then exits 0.

All other findings are `verified`: component bodies moved byte-for-byte (0
non-import diffs / 117 files); the kit imports no `@/` / app store (the one
`Stores.` hit is a decoupling doc comment); the `DivScrollY`→`Stores.AppLayout`
coupling is severed with a render-equivalent store-free scroller (dialog never used
`nativeFlow`); the vendored `cn`/`use-mobile`/`DivScrollX` are diff-identical to
their app originals; the slice is deleted from ziee (single source); the workspace
symlink is wired; the token SoT is not forked.

The one `acknowledged` finding (**tooling-followup**, F1-14) is a genuine
out-of-scope follow-up (kit-scanning `gen-*.mjs` scripts still point at the old
path — relevant only to the full `npm run check`, NOT the tsc-clean F1 gate), not
a confirmed defect in the extraction itself.

Re-audit of the post-fix diff surfaced no new issues.

**New confirmed findings:** 0
