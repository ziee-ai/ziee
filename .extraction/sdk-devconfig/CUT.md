# Chunk sdk-devconfig — shared dev-config + quality-gate tooling (CUT manifest)

Extracts ziee's shared **dev-config + design-token quality-gate tooling** into a new
`@ziee/config` SDK package so ANY app gets consistent linting, type-checking, and
design-token enforcement by *extending* the configs + running the parameterized lints
against its own tree.

**This chunk is ADDITIVE + a partial MOVE.** The new SDK package is additive; on the
ziee side, seven now-redundant `ui/scripts/*.mjs` lints/generators are MOVED into the
SDK (deleted from ziee, whose `package.json` now invokes the SDK copies), and ziee's
`biome.json` / `tsconfig.json` / root syncpack config are converted to *consume* the
shared base. The equivalence anchor is therefore **backward-compat**: ziee's
`ui/` + `desktop/ui/` `tsc --noEmit` stay `0`, and every config-layer `check` step
(`lint:guardrails` / `lint:colors` / `lint:settings-field` / `lint:adjacent-inline` /
`lint:logical-direction` / `lint:tooltip-placement` / `check:design-spec` /
`check:kit-manifest`) produces byte-identical output — proven by running the SDK
entrypoints against ziee's src and diffing vs the pre-change baseline.

No Rust, no `emit_ts`, no `openapi.json`, no `api-client/types.ts` — pure
config/tooling, so the generated golden files are untouched by construction.

## New SDK package `@ziee/config` (all NEW files, in the `sdk` submodule)

Pure configs (apps `extends`):
- `sdk/packages/config/biome.base.json` — generic Biome linter+formatter preset (the
  `recommended:false` rule sets, formatter, JS style, globals). Excludes ziee's
  app-specific `noRestrictedImports` (antd bans) + grit-plugin overrides + desktop
  file-excludes — those stay in the app.
- `sdk/packages/config/tsconfig.base.json` — strict generic `compilerOptions` (no app
  `paths`/`include`/`references`).
- `sdk/packages/config/syncpack.base.mjs` — shared version policy (`typescript` `~`,
  else `^`, one version everywhere) + `defineSyncpack({source, versionGroups})` helper.

Parameterized design-contract lints/generators (`--root` / `--css` / `--barrel` …):
- `sdk/packages/config/src/lint/roots.mjs` — shared `--root=<dir>` (repeatable) /
  `--<flag>` arg parsing, resolved vs CWD.
- `sdk/packages/config/src/lint/hardcoded-colors.mjs` — no hardcoded colors (design-contract).
- `sdk/packages/config/src/lint/settings-field.mjs` — settings controls in a kit Field.
- `sdk/packages/config/src/lint/adjacent-inline.mjs` — adjacent inline pills need a gap.
- `sdk/packages/config/src/lint/logical-direction.mjs` — RTL logical-direction (diff-scoped;
  `--path-include` parameterizes the monorepo path filter).
- `sdk/packages/config/src/lint/tooltip-placement.mjs` — uniform tooltip side (advisory).
- `sdk/packages/config/src/lint/design-spec.mjs` — generate/`--check` `DESIGN_SYSTEM.md`
  from the shadcn CSS token source (`--css`/`--out`/`--app-name`).
- `sdk/packages/config/src/lint/kit-manifest.mjs` — generate/`--check` the kit's
  `KIT_MANIFEST.md` (`--barrel`/`--out`/`--tsconfig-dir`).

Composable runner + package plumbing:
- `sdk/packages/config/src/check.mjs` — `ziee-check` composable gate (tsc + biome
  guardrail + token lints + design-spec + kit-manifest), parameterized + step-toggleable.
- `sdk/packages/config/package.json` — `exports` (`./biome`, `./tsconfig`, `./syncpack`,
  `./check`, `./lint/*`) + `bin` (`ziee-check`, `ziee-lint-*`, `ziee-design-spec`,
  `ziee-kit-manifest`); `typescript` optional peer.
- `sdk/packages/config/README.md` — the `extends` API + lint catalog.
- `sdk/packages/config/scripts/config.test.mjs` — smoke (4 tests) incl. the
  parameterization proof (arbitrary `--root`: clean passes, violation fails).

## ziee-side changes (main repo — MOVE + consume; staged, not pushed)

- delete: `src-app/ui/scripts/{lint-hardcoded-colors,lint-settings-field,lint-adjacent-inline,lint-logical-direction,lint-tooltip-placement,gen-design-spec,gen-kit-manifest}.mjs` — moved into `@ziee/config`.
- edit: `src-app/ui/package.json` — the 9 corresponding `scripts` now invoke the SDK copies with ziee's explicit paths (`--root=src --root=../desktop/ui/src`, `--css src/index.css --out ../../DESIGN_SYSTEM.md`, `--barrel ../../sdk/packages/kit/src/index.ts`). `lint:icon-action` + `lint:native-scroll` KEPT (ziee-local).
- edit: `src-app/ui/biome.json` — `extends: ["../../sdk/packages/config/biome.base.json"]`, keeping only the app-specific `files.includes`, `noRestrictedImports`, and grit-plugin overrides.
- edit: `src-app/ui/tsconfig.json` — `extends: "../../sdk/packages/config/tsconfig.base.json"`, keeping only `lib` (ES2020 narrowing), `paths`, `include`, `exclude`, `references`.
- MOVE: `.syncpackrc.json` → `.syncpackrc.mjs` (`export default defineSyncpack({source, versionGroups})` importing the shared policy; app-specific desktop/test versionGroups retained).
- `package-lock.json` — `npm install` registering the `@ziee/config` workspace.

## Kept in ziee (ziee-app-specific — NOT extracted)

- `src-app/ui/scripts/lint-icon-action.mjs` — encodes a **curated English-word→lucide-glyph
  opinion** + a gallery-scoped allowlist + is wired into the gallery detector-acceptance
  harness; not derivable from the SDK design system. (See TRANSFORMS Decision.)
- `src-app/ui/scripts/lint-native-scroll.mjs` — not part of `check`, not requested; left as-is.
- The gallery/visual checks (`check:gallery-*`, `check:state-matrix`, `check:testid-registry`,
  `check:overlay-registry`, `runtime-health`, …) — a separate `@ziee/gallery` extraction owns these.

## Bonus fix

ziee's `check:kit-manifest` was **already broken** on this branch (the kit had moved to
`@ziee/kit`, leaving its script pointing at a non-existent `src/components/ui/index.ts`
barrel — exit 1). Re-pointing it at the SDK kit barrel via the parameterized tool makes
it PASS again (`KIT_MANIFEST.md up to date`).
