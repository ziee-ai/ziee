# Chunk F1 — DRIFT scan (round 1)

Drift = any place moving the kit into `@ziee/kit` could diverge from
pre-extraction source/behavior/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. **Component source byte-preservation.** A
  per-file `diff` of every moved file against its `HEAD` original reports **0
  non-import differing lines** across 117 files (22 byte-identical, 95
  import-only). The barrel differs only in its line-1 doc comment. Bodies of every
  kit + shadcn component are byte-for-byte; only `@/…` import specifiers were
  rewritten to kit-relative. The chunk's core requirement.

- **DRIFT-1.2** — verdict: none. **No `@/` back-import remains in the kit.** Grep
  of `sdk/packages/kit/src` for `from '@/…'` / `import('@/…')` / `Stores.` /
  `@/core` / `@/modules` / `@/api-client` / `@ziee/framework` / `@ziee/ui-core`
  returns **zero** (the single `Stores.` hit is a doc comment in
  `div-scroll-y.tsx` explaining the decoupling). The package names no app type,
  store, or alias — fully domain-agnostic.

- **DRIFT-1.3** — verdict: none. **Vendored micro-deps are faithful.**
  `src/lib/utils.ts` (`cn`), `src/hooks/use-mobile.ts` (`useIsMobile`), and
  `src/internal/div-scroll-x.tsx` are `diff`-identical to their app originals.
  `src/internal/div-scroll-y.tsx` is the app `DivScrollY` minus the
  `Stores.AppLayout` read + the `nativeFlow` branch; the kit's only consumer
  (`kit/dialog.tsx`) never passes `nativeFlow`, so the removed branch produced no
  output → render-equivalent for the dialog (TRANSFORMS T-4).

- **DRIFT-1.4** — verdict: none. **No orphan / stale import in ziee.** After the
  quote-anchored repoint + `git rm` of the slice, grep of `ui/src`, `ui/tests`,
  and `desktop/ui/src` for a `@/components/ui` import specifier returns **none**;
  the two `story.tsx` mentions are prose comments, not imports. `ui/tests/e2e/
  testid.ts`'s relative path into the moved `testIds.generated` is repointed to
  `@ziee/kit/testIds.generated`.

- **DRIFT-1.5** — verdict: none. **Module resolution — all three tsc gates green.**
  `@ziee/kit` standalone `tsc --noEmit` = **0**; ziee `ui/` `tsc --noEmit` = **0**;
  `desktop/ui/` `tsc --noEmit` = **0**. The extensionless-`exports`-subpath tsc
  limitation is handled by consumer `paths` (T-6), matching the repo's existing
  `@ziee/ui-core` split; runtime bundlers use the package `exports`.

- **DRIFT-1.6** — verdict: none. **npm-submodule wiring proven.** `npm install`
  at the worktree root created `node_modules/@ziee/kit` → `sdk/packages/kit`
  (symlink), with every third-party dep hoisted to the root `node_modules` (react,
  overlayscrollbars-react, lucide-react, base-ui, …). This is the F1 milestone:
  the `sdk/packages/*` npm workspaces resolve as symlinks.

- **DRIFT-1.7** — verdict: none. **Design-token SoT not forked.** `tokens.css` is
  an additive byte-identical extract of `index.css` 47–205; `ui/src/index.css`
  (the `gen:design-spec` SoT) is UNCHANGED, so ziee's build + the design-spec
  check are unaffected. The kit copy is a self-contained convenience for future
  consumers, not a second authority.

- **DRIFT-1.8** — verdict: informational (out of the tsc gate). **Kit-scanning
  build tooling still points at the old path.** `ui/scripts/gen-kit-manifest.mjs`
  + `gen-testid-registry.mjs` + `classify-gallery-coverage.mjs` scan
  `src/components/ui`, so the full `npm run check` sub-gates `check:kit-manifest`/
  `check:testid-registry` would need their scan roots repointed to the kit. This
  is explicitly OUT of scope for F1's tsc-clean gate (this chunk is
  wire-irrelevant); noted in BOUNDARY as a follow-up. It does not affect
  `tsc --noEmit` in any of the three surfaces.

**Unresolved drifts:** 0
