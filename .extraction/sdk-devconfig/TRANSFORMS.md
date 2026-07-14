# Chunk sdk-devconfig — TRANSFORMS (every non-trivial change + Decision)

## T-PARAM-1 — parameterize the lint scan roots (`roots.mjs`)

Each extracted lint hard-coded `const ROOTS = [resolve(HERE,'../src'),
resolve(HERE,'../../desktop/ui/src')]`. Replaced with `parseRoots()` (shared
`roots.mjs`): reads `--root=<dir>` / `--root <dir>` (repeatable), resolved vs the
process CWD, defaulting to `['src']`. — **why:** the SDK lint must scan the *consuming
app's* tree, not a path relative to the SDK file. When ziee invokes with
`--root=src --root=../desktop/ui/src` from `src-app/ui`, the resolved absolute dirs
are exactly the originals → byte-identical behavior (the report uses
`path.relative(process.cwd(), …)`, same CWD → same output). Proven: all 5 lints emit
byte-identical stdout vs baseline.

### Decision (root parameterization)
**Q:** default roots, or require `--root`? **R:** default to `['src']` (the common
single-tree app) but let a monorepo pass multiple `--root`s. ziee passes both its UI
trees explicitly, so nothing about ziee's coverage changes. Zero TBD.

## T-PARAM-2 — `logical-direction` path filter (`--path-include`)

`isScanned` hard-coded `p.includes('src-app/ui/src/') || p.includes('src-app/desktop/ui/src/')`
(ziee's monorepo layout). Parameterized via `--path-include=<substr>` (repeatable),
defaulting to those two substrings. — **why:** this lint is diff-scoped (git merge-base),
so it can't use `--root`; the path filter is the app-tree seam. Default reproduces ziee.

## T-PARAM-3 — `design-spec` over `--css`/`--out`/`--app-name`

`gen-design-spec` hard-coded `CSS = ../src/index.css` and `OUT = ../../../DESIGN_SYSTEM.md`
and the literal title `# Ziee Design System`. Parameterized: `--css` (token source),
`--out` (write/check target), `--app-name` (title; default `Ziee`). — **why:** the
MECHANISM (parse `@theme inline`/`:root`/`.dark` → token table + radius, compose the
contract) is 100% generic; only the paths + app name are app-specific. With ziee's
paths + `app-name Ziee` (default), the generated markdown is **byte-identical** to the
committed `DESIGN_SYSTEM.md` (proven by diff).

### Decision (design-spec editorial prose)
**Q:** parameterize the whole prose, or ship ziee's prose as the default template?
**R:** ship the prose as the shipped default (parameterizing only paths + app name).
The token table/radius/spacing are derived from CSS (generic); the surrounding prose
(spacing rhythm, Field-not-flex, forbidden-patterns) is genuinely good *generic*
design-system guidance and doubles as a sensible default for a new app. The only
residual ziee-isms are a few doc-string paths in the prose (`@/components/ui`,
`scripts/lint-hardcoded-colors.mjs`) preserved verbatim for byte-identity; a future
app can regenerate with its own `--app-name` and edit those references. Keeping ziee
byte-identical (the backward-compat contract) outranked fully genericizing the prose in
this chunk. Zero TBD.

## T-PARAM-4 — `kit-manifest` over `--barrel`/`--out`/`--tsconfig-dir`

`gen-kit-manifest` derived `BARREL`/`OUT` from its own file location + `ts.findConfigFile`
from that ROOT. Parameterized: `--barrel` (the kit's public barrel), `--out` (defaults
next to the barrel), `--tsconfig-dir` (tsconfig resolution root; defaults to the barrel's
dir, walking up). — **why:** the kit moved to `@ziee/kit` (prior chunk), so the manifest
must be generated from `sdk/packages/kit/src/index.ts` against `sdk/packages/kit/tsconfig.json`.
The header string (incl. `@/components/ui`) is preserved verbatim so the committed
`sdk/packages/kit/src/KIT_MANIFEST.md` (67 components) stays up-to-date (`--check` PASS).

## T-BIOME-1 — extract the generic Biome preset (`biome.base.json`)

`biome.base.json` = ziee's `biome.json` MINUS the app-specific bits: the
`noRestrictedImports` antd bans, the two grit-plugin overrides (paths into ziee's
`./biome-plugins/`), and the `files.includes` desktop excludes. ziee's `biome.json`
now `extends` the base (relative path) and keeps ONLY those app-specific pieces. —
**why:** the generic rule sets + formatter + JS style + globals are reusable; the antd
ban + grit plugins name ziee's own migration/paths. Proven identical: `lint:guardrails`
(`--only=style/noRestrictedImports`) output identical (modulo run-time-ms jitter), and
`biome check ./src` identical (`Checked 952 files … Found 1299 errors … 123 warnings`,
same exit 1 pre-existing).

### Decision (Biome extends vs ship-only)
**Q:** wire ziee onto the base, or ship-only? **R:** wire — empirically verified the
`extends` merge is a no-op for lint results (base ⊆ ziee's prior rules; the antd rule
`lint:guardrails` needs stays in ziee's own config, so `--only` is unaffected). Used a
**relative** `extends` path (`../../sdk/packages/config/biome.base.json`) rather than the
`@ziee/config/biome` package specifier, because relative resolution is unconditionally
supported and avoids a node-resolution dependency during lint. A published app uses the
package specifier. Zero TBD.

## T-TSCONFIG-1 — extract the strict base (`tsconfig.base.json`)

`tsconfig.base.json` = the generic `compilerOptions` (target/module/moduleResolution/
strict/noUnused*/jsx/…), NO app `paths`/`include`/`references`. ziee's `tsconfig.json`
now `extends` it (relative) and keeps `lib` (ES2020 narrowing — child wins), `paths`,
`include`, `exclude`, `references`. — **why:** compilerOptions are the reusable strict
contract; path mappings + include globs are per-app. Proven: `ui/` `tsc --noEmit` exit 0
(clean, matching baseline); `desktop/ui/` left untouched → still exit 0.

## T-SYNCPACK-1 — shared version policy (`syncpack.base.mjs`) + ziee consumes it

`syncpack.base.mjs` exports `semverGroups` (typescript `~`, else `^`), the catch-all
`sameRangeVersionGroup`, and `defineSyncpack({source, versionGroups})` (app exceptions
placed BEFORE the catch-all). ziee's `.syncpackrc.json` → `.syncpackrc.mjs` importing it,
retaining its desktop-plugin + test-tooling versionGroups. — **why:** the semver policy +
one-version-everywhere rule are reusable; the source globs + desktop/test exceptions are
ziee's. Proven: `npx syncpack lint` output byte-identical (same pre-existing exit 1 from
package.json *formatting*, unrelated to versions — all version/semver groups still valid).

### Decision (syncpack JSON→mjs)
**Q:** JSON stays (ship-only) or convert to mjs (consume)? **R:** convert — verified the
`.mjs` config composes byte-identically AND syncpack resolves `@ziee/config/syncpack`.
The only tradeoff is a load-order dependency (the workspace must be installed before
`syncpack` runs), which is already true of every workspace tool. Zero TBD.

## T-CHECK-1 — composable `ziee-check` runner

`src/check.mjs` runs `tsc → biome guardrail → colors → settings-field → adjacent-inline →
logical-direction → tooltip-placement → design-spec → kit-manifest` in sequence,
parameterized over the app tree (`--root`/`--css`/`--design-out`/`--kit-barrel`/…) and
step-toggleable (`--no-tsc`, `--no-kit-manifest`, …). — **why:** a new app references ONE
bin instead of hand-maintaining a 10-step `&&` chain. It shells `tsc`/`biome` via
`npx --no-install` so the app's own versions run. Gallery/visual checks are deliberately
OUT of scope (a separate `@ziee/gallery` layer). Proven: runs 9 steps green against ziee.

## Guardrail-lint triage — GENERIC (extracted) vs ZIEE-SPECIFIC (kept)

The distinguishing criterion: **does the lint enforce a structural invariant derivable
purely from the SDK design system (tokens, kit components, Tailwind/RTL conventions),
with app data being only a parameterizable path?**

| lint | verdict | rationale |
|---|---|---|
| `lint-hardcoded-colors` | **GENERIC → extracted** | enforces the kit's semantic token classes vs raw Tailwind hues; pure design-contract. |
| `lint-settings-field` | **GENERIC → extracted** | kit `Field`/`FormField` composition invariant. |
| `lint-adjacent-inline` | **GENERIC → extracted** | flex-gap spacing invariant; pill set = kit component names. |
| `lint-logical-direction` | **GENERIC → extracted** | Tailwind logical-direction (RTL) is universal; monorepo path filter parameterized. |
| `lint-tooltip-placement` | **GENERIC → extracted** | kit `Tooltip`/`Button tooltipSide` uniformity (advisory). |
| `lint-icon-action` | **ZIEE-SPECIFIC → kept** | needs a **curated English-word→lucide-glyph MAP** (editorial content, NOT derivable from tokens/kit API) + a gallery-scoped `icon-action-allowlist.json` + is an instrument of the gallery detector-acceptance harness. |
| `lint-native-scroll` | **kept** | not wired into `check`, not in the requested set; left as-is. |

### Decision (icon-action stays)
**Q:** extract icon-action too (its `--root` seam already exists)? **R:** keep it in
ziee. Unlike the five structural lints, its correctness rests on a hand-curated
action→glyph opinion + a gallery allowlist — app content, not an SDK design-system rule.
Extracting it would either ship ziee's editorial map to every app (wrong default) or
require a config surface out of scope for this chunk. Zero TBD.
