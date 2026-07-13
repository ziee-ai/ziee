# Gate-precedent survey — the `gen + --check` idiom in `src-app/ui/`

Goal: extract the EXACT pattern of ziee's "generate a committed artifact / `--check` drift-gate"
scripts so a new completeness gate can mirror them. All paths below are under
`src-app/ui/` unless noted. Every one of these scripts runs under **plain Node**
(`node scripts/X.mjs`), NOT vite — so `import.meta.glob` is unavailable and each
discovers its inputs by hand (fs-walk + regex, ts-morph AST, or TS compiler API).

---

## The shared idiom (all six scripts)

Every generator is a single `.mjs` with three (sometimes four) modes selected by argv:

```
node scripts/gen-X.mjs            → WRITE  (regenerate the committed artifact + println count)
node scripts/gen-X.mjs --check    → CHECK  (regenerate IN MEMORY, byte-compare vs committed file,
                                            exit 1 on mismatch; often + extra semantic gates)
node scripts/gen-X.mjs --scaffold → SCAFFOLD (opt-in: append missing keys to a hand-file as TODOs)
node scripts/gen-X.mjs --list     → LIST   (human summary; overlay-registry only)
```

Core drift mechanism, verbatim shape in EVERY script:

```js
const body = render(...)                 // build the artifact string in memory
if (check) {
  const cur = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf-8') : ''
  if (cur.trim() !== body.trim()) {      // .trim() byte-compare (JSON: JSON.stringify(x,null,2))
    console.error('<OUT> is stale — run `<gen cmd>` and commit.')
    process.exit(1)
  }
  console.log('<OUT> up to date ...')
} else {
  fs.writeFileSync(OUT, body)
  console.log(`Wrote ${OUT} ...`)
}
```

So "drift" = the committed file no longer equals what the generator would produce
from the current source tree. `--check` NEVER writes; it only compares + exits.

Two of the four primary gates add a SECOND, semantic gate on top of the byte-compare
(a "missing set" / "coverage" gate), which is the part the new completeness gate cares
about most.

Two enforcement styles are used across the family:
- **Runtime `--check` gate** (override, overlay): the `.mjs` itself computes the missing
  set and `process.exit(1)`s. Runs as an `npm run check:*` step.
- **tsc `satisfies Record<Union, Entry>` gate** (gallery-coverage, state-matrix): the
  generator only owns a generated `type`/union; a hand-authored `.ts` file declares
  `X satisfies Record<GeneratedUnion, Entry>`, so a missing key is a **compile error**
  caught by the leading `tsc` in `npm run check`. The `--check` byte-compare just keeps
  the union fresh so tsc sees new members.

---

## `npm run check` wiring (from `package.json`)

The `check` script (order preserved) is:

```
tsc
&& lint:guardrails && lint:colors && lint:settings-field && lint:adjacent-inline
&& lint:icon-action && lint:logical-direction && lint:tooltip-placement
&& check:kit-manifest        (node scripts/gen-kit-manifest.mjs --check)
&& check:testid-registry     (node scripts/gen-testid-registry.mjs --check)
&& check:design-spec         (node scripts/gen-design-spec.mjs --check)
&& check:gallery-coverage    (node scripts/gen-gallery-coverage.mjs --check)
&& check:gallery-crawl       (node scripts/gen-crawl-cassette.mjs --check)
&& gallery:check-fixtures    (node scripts/check-gallery-fixtures.mjs)
&& check:state-matrix        (node scripts/gen-state-matrix.mjs --check)
&& check:overlay-registry    (node scripts/gen-overlay-registry.mjs --check)
&& check:override-registry   (node scripts/gen-override-registry.mjs --check)
```

Key ordering fact: **`tsc` runs FIRST**. That is what makes the `satisfies Record<…>`
gates (gallery-coverage + state-matrix) fire — the `--check` byte-compare for those two
only keeps the generated union in sync so tsc has the right members to complain about.
Each generator has a paired `gen:X` script (no `--check`) to refresh the artifact.

A NEW completeness gate mirrors this by adding two package.json scripts —
`gen:<name>` and `check:<name>: node scripts/gen-<name>.mjs --check` — and appending
`&& npm run check:<name>` to the end of `check`.

---

## Script-by-script

### 1. `gen-override-registry.mjs` — THE PRIMARY PRECEDENT (runtime missing-set gate)

**Generates:** `src/core/overrides/OVERRIDE_MANIFEST.md` (committed, git-tracked). A
Markdown doc with an `<!-- AUTO-GENERATED … DO NOT EDIT -->` header + four tables:
declared element-level seams, whole-file `.desktop.tsx` overrides, approved raw shadows,
desktop-exclusive modules. Header line + per-section counts.

**Source-of-truth it reads (WITHOUT vite):** raw `fs` recursive `walk()` over TWO trees —
`../src` (core `ui/src`) and `../../desktop/ui/src` (desktop) — reading every non-test
`.tsx|.ts`, then REGEX over source text:
- `interface UIOverrides { … }` augmentations → **declared** seams. Extracted by a
  brace-balanced depth scan (`topLevelSeamKeys`), taking only depth-0 quoted keys (a seam
  value can be a multi-line object type). Comments are stripped first (`stripComments`) so
  JSDoc examples don't pollute.
- `<Seam id="…">` + `useOverride('…')` → **used** seams (core).
- `registerOverride('…')` → **registered** overrides (desktop).
- files matching `\.desktop\.(tsx|ts)$` → whole-file overrides; sibling existence checked
  with `fs.existsSync(base + '.tsx'|'.ts')`.
- A separate `walkDesktopSrc` + `enumerateDesktopFiles` splits every desktop-tree file into
  **raw shadows** (a `ui/src` sibling of the same relPath exists) vs **desktop-exclusive**
  (no sibling).

**A second committed source of truth (the allow-list):**
`src-app/desktop/ui/OVERRIDE_EXCEPTIONS.md` — parsed by `parseShadowExceptions` with the
line regex `^-\s*SHADOW-EXCEPTION:\s*(\S+)\s+[—-]\s.*\[approved:`. Approved shadow paths
form the allow set.

**What `--check` does (two gates):**
1. **Semantic missing-set gate** via `computeDrift(declared, registered, desktopFiles)`:
   - `deadOverrides` = `registerOverride` keys with NO declared seam → FAIL
     (`override drift: registerOverride() for undeclared seam(s): …`).
   - `orphanDesktopFiles` = `*.desktop.*` with no core sibling → FAIL.
   - `unaccountedShadows` = raw shadows NOT in the exceptions allow-list → FAIL
     (`override drift: N RAW whole-file shadow(s) not migrated and not approved: …`).
   - `unregisteredSeams` = declared-but-not-registered = LEGITIMATE (web-only) → reported, NOT failed.
2. **Byte-compare drift gate:** `cur.trim() !== body.trim()` → sets `failed`,
   `OVERRIDE_MANIFEST.md is stale — run \`node scripts/gen-override-registry.mjs\` and commit.`
   Any failure ⇒ `process.exit(1)`; success prints `override registry OK — N seam(s), …`.
   In WRITE mode it `fs.writeFileSync(OUT, body)` then STILL `process.exit(1)` if a drift
   condition held (you can't regenerate your way out of a dead override).

**Wired:** `check:override-registry` = `node scripts/gen-override-registry.mjs --check`,
LAST in `npm run check`. Paired `gen:override-registry`.

**B6 satisfied (permanent committed path):** both the OUTPUT (`src/core/overrides/OVERRIDE_MANIFEST.md`)
and the allow-list SOURCE (`src-app/desktop/ui/OVERRIDE_EXCEPTIONS.md`) live in the product
tree and are git-tracked. The code carries an explicit inline comment at the allow-list read:
> "PERMANENT source of truth for approved exceptions — a committed product-tree file (NOT
> `.lifecycle/`, which is STRIPPED at merge, which would make the gate find zero exceptions
> and fail `npm run check` forever on main)."
The exceptions file's own header repeats this: it lives in the product tree "NOT under
`.lifecycle/`, which is stripped at merge, precisely so the gate keeps working on `main`."
This is the exact B6 rule the new gate must honor.

---

### 2. `gen-overlay-registry.mjs` — runtime missing-set gate (JSON output + allow-list)

**Generates:** `src/dev/gallery/overlay-registry.generated.json` (committed, git-tracked).
Shape: `{ generatedBy, counts:{total,hosts,triggers,wiredOpen}, hosts:[…], triggers:[…] }`,
each surface = `{ surface, class:'host'|'trigger', primitives:[…], occurrences:[…] }`.

**Source-of-truth it reads (WITHOUT vite):** fs-`walk()` over TWO roots — `src/modules`
(skipping `module.tsx`) and `src/components/ui` — reading `.tsx` (not test/stories). Per file:
- `importedPrimitives(src)`: regex every `import { … } from '…'` whose source is a kit path
  (`@/components/ui[/…]` or the app-layout `Drawer`), collecting bound overlay-primitive names
  (Dialog/Drawer/Sheet/Modal/Popover/AlertDialog/Confirm/Popconfirm). A local same-named
  component from elsewhere is ignored.
- `scanOccurrences`: for each imported primitive, regex `<Prim …>` / `<Prim … />` and classify
  `controlled` (has `open=`/`open`/`isOpen`/`visible=`) vs `trigger` (self-opening). A surface
  with ≥1 controlled occurrence is class `host`, else `trigger`.

Two more committed sources feed the gate:
- **What IS covered — wired:** `wiredSurfaces()` reads `src/dev/gallery/overlays.tsx` and regexes
  every `surface:\s*'([^']+)'` → the set rendered OPEN in the gallery.
- **Allow-list:** `src/dev/gallery/overlay-allowlist.json` (`{ hosts:{surface:reason}, triggers:{…} }`),
  loaded by `loadAllowlist()`.

**What `--check` does (two gates):**
1. **Byte-compare:** `cur.trim() !== JSON.stringify(registry, null, 2).trim()` →
   `overlay-registry.generated.json is stale — run … and commit.` → `exit(1)`.
2. **Semantic missing-set gate** via `statusOf(s)`: `wired`→ok, else allow-listed→ok, else
   `MISSING`. Fails on any `missingHosts` / `missingTriggers` (an overlay never rendered open
   and not allow-listed) AND on `staleAllow` (an allow-list entry now wired or gone — keeps the
   list honest). Distinct per-bucket error blocks (`overlay HOST(s) are never rendered open…`,
   `self-opening overlay(s) … have no render coverage`, `stale allow-list entry/entries`) →
   `exit(1)`. Success: `overlay gate OK — N surfaces (…): W wired open, A allow-listed.`

**Wired:** `check:overlay-registry`, second-to-last in `check`.
**B6:** output JSON + `overlay-allowlist.json` + `overlays.tsx` are all committed product-tree files.

---

### 3. `gen-gallery-coverage.mjs` — tsc `satisfies Record<…>` gate (union generator)

**Generates:** `src/dev/gallery/galleryCoverage.generated.ts` (committed). Exports
`GALLERY_SURFACES` (a `[… ] as const` array of every surface id) + `type GallerySurface =
(typeof GALLERY_SURFACES)[number]`. It owns ONLY the union/denominator; the Record is authored
by humans.

**Source-of-truth it reads (WITHOUT vite):** fs-`walk()` over `src/modules` (skip `module.tsx`)
+ `src/components/ui`, collecting every `.tsx` except `*.{test,stories,desktop}.tsx`. Surface id
= path from `src/` without extension. That's the whole discovery — a pure fs glob.

**What IS covered:** the hand-maintained `src/dev/gallery/coverage.ts` declares
`GALLERY_COVERAGE satisfies Record<GallerySurface, Coverage>`. A surface with no entry is a
**compile error** — the coverage gate is really tsc. `coverage.ts` provides escape hatches
(`page`/`story`/`via`/`allow`/`pending`/`static`/`nonvisual`) so every surface is at least
`pending`.

**What `--check` does (byte-compare + a SECOND semantic gate):**
1. Byte-compare of the generated union: stale → `galleryCoverage.generated.ts is stale — run
   \`npm run gen:gallery-coverage\` and commit.` → `exit(1)`.
2. **Required-state-set gate:** `parseCoverage()` regexes `coverage.ts` entries into
   `{id:{kind,states}}`; for each kind in `REQUIRED` (`data-page`/`table`→loaded,empty,error;
   `form`→empty,filled,invalid; `overlay`→open) any missing state → `stateFailures` →
   `exit(1)` (`N surface(s) missing required states …`).
3. Non-fatal: lists `pending` surfaces as tracked TODO.
`--scaffold` (opt-in, not in `check`) appends missing surfaces to `coverage.ts` as
`pending('auto-scaffold — needs review')` at a `// <<< scaffold-insert >>>` marker.

**Wired:** `check:gallery-coverage`. **B6:** generated union + `coverage.ts` are committed.

---

### 4. `gen-state-matrix.mjs` — tsc `satisfies Record<…>` gate (AST-driven union)

**Generates (committed):** `src/dev/gallery/stateMatrix.generated.ts` (the `STATE_MATRIX`
record + `PANEL_RENDERERS` + `SLOT_REGISTRATIONS` + `type RequiredState` = one
`"surface:state"` member per named required state + `REQUIRED_STATE_KEYS`) AND
`src/dev/gallery/STATE_MATRIX.md` (human review artifact).

**Source-of-truth it reads (WITHOUT vite):** a **ts-morph `Project`** loaded from
`tsconfig.json` (`skipAddingFilesFromTsConfig:true`), then `addSourceFilesAtPaths` over globs
`modules/**/*.tsx` + `components/ui/**/*.tsx`. It does a real AST pass (NOT regex) to extract,
per surface: conditional renders (ternary / `&&` / early `if(){return}`) classified
loading/error/empty/branch by identifiers in the condition; overlay open-triggers (a
Dialog/Drawer/… JSX element with a controlled `open|visible|defaultOpen|isOpen` attr);
`registerPanelRenderer('x')` calls; and slot registrations (`registerSlot('x')` + object-literal
`slots: { sidebarContent: … }` keys in `module.tsx`). Named signal kinds map to required gallery
states (`KIND_TO_STATE`); generic `branch` gets no key (proven by runtime coverage, not tsc).

**What IS covered:** hand-maintained `src/dev/gallery/stateCoverage.ts` declares
`STATE_COVERAGE satisfies Record<RequiredState, StateCoverageEntry>` — a newly-extracted state
with no entry is a **compile error**; entries are `{ via }` (delivered) or `{ skip, reason }`
(allow-listed gap).

**What `--check` does:** byte-compare BOTH generated files (`stateMatrix.generated.ts` +
`STATE_MATRIX.md`); stale → `state matrix is stale — run \`npm run gen:state-matrix\` and commit …`
→ `exit(1)`. Then (report + secondary gate) `parseCoverageKeys()` regexes `stateCoverage.ts`
keys; any `requiredStateKeys` not present → `exit(1)` with `run … --scaffold`. (Freshness of the
Record is primarily enforced by tsc via `satisfies`.) `--scaffold` appends missing keys with a
semantically-correct default (`{ via:'page-state-mode' }` / `{ via:'overlay' }` / `{ skip,reason }`)
at the `// <<< state-scaffold-insert >>>` marker.

**Wired:** `check:state-matrix`. **B6:** both generated files + `stateCoverage.ts` are committed.

---

### 5–6. `gen-kit-manifest.mjs` + `gen-testid-registry.mjs` — the minimal shared idiom (skim)

Both are pure byte-compare drift gates (NO missing-set gate):
- **kit-manifest:** discovers inputs via the **TypeScript compiler API** — builds a `Program`
  from `tsconfig` + the kit barrel `src/components/ui/index.ts`, walks exported `*Props` symbols,
  emits `KIT_MANIFEST.md`. `--check` = `existing.trim() !== md.trim()` →
  `KIT_MANIFEST.md is stale — run \`node scripts/gen-kit-manifest.mjs\` and commit.` → `exit(1)`.
- **testid-registry:** discovers inputs via fs-`walk()` over `ui/src` + `desktop/ui/src` (skipping
  `src/dev`), regex `data-testid\s*[=:]\s*["']([^"']+)["']` → sorted set → emits
  `testIds.generated.ts` (`TEST_IDS` const array + `KnownTestId` union). Same byte-compare
  `--check`: `testIds.generated.ts is stale — run \`npm run gen:testid-registry\` and commit.`
These two prove the baseline: fs-glob-or-TS-API discovery → render string → `.trim()` compare.

---

## Discover-without-vite technique per script (the critical constraint)

| Script | Input discovery (NO `import.meta.glob`) | Output | `--check` = |
|---|---|---|---|
| gen-override-registry | fs `walk()` over `ui/src` + `desktop/ui/src`; regex `interface UIOverrides`/`<Seam>`/`useOverride`/`registerOverride`/`.desktop.` ; parse `OVERRIDE_EXCEPTIONS.md` allow-list | `OVERRIDE_MANIFEST.md` | drift byte-compare **+** `computeDrift` missing-set (dead overrides / orphans / unaccounted shadows) |
| gen-overlay-registry | fs `walk()` over `src/modules`+`src/components/ui`; regex kit imports + `<Prim>` occurrences; regex `overlays.tsx` for wired `surface:`; read `overlay-allowlist.json` | `overlay-registry.generated.json` | drift byte-compare **+** MISSING-set (unwired & unlisted hosts/triggers) + stale-allow |
| gen-gallery-coverage | fs `walk()` over `src/modules`+`src/components/ui`; path→id | `galleryCoverage.generated.ts` (union) | drift byte-compare **+** required-state gate; the coverage gate itself is **tsc** `satisfies Record<GallerySurface,…>` on `coverage.ts` |
| gen-state-matrix | **ts-morph AST** over `modules/**`+`components/ui/**` `.tsx` (conditional renders / overlay / panel / slot) | `stateMatrix.generated.ts` + `.md` (`RequiredState` union) | drift byte-compare (2 files); coverage gate is **tsc** `satisfies Record<RequiredState,…>` on `stateCoverage.ts` |
| gen-kit-manifest | **TS compiler API** Program over kit barrel `*Props` | `KIT_MANIFEST.md` | drift byte-compare only |
| gen-testid-registry | fs `walk()` + regex `data-testid` over both trees | `testIds.generated.ts` | drift byte-compare only |

Three discovery families available at check-time under plain Node:
1. **fs recursive `walk()` + regex over source text** (override, overlay, testid) — cheapest.
2. **ts-morph `Project`** loaded from `tsconfig` via `addSourceFilesAtPaths(glob)` — real AST
   (state-matrix). `ts-morph` is a devDependency.
3. **`typescript` compiler API** `createProgram` (kit-manifest) — for type/symbol introspection.
None use vite; `import.meta.glob` never appears. A committed generated JSON/TS can ALSO be read
back as an input (state-matrix reads `coverage.ts`; overlay reads its own inputs) — this is the
"read a generated product-tree file" option.

---

## REUSABLE TEMPLATE — the `gen-override-registry.mjs` mechanism as a completeness gate

The precedent's completeness gate is a **set-difference: `MUST_COVER \ IS_COVERED = MISSING`**,
where BOTH sets are discovered from committed product-tree files, and a non-empty `MISSING`
(minus an allow-list) `process.exit(1)`s. Full mechanism, mapped to the four moving parts:

**A. Enumerate what MUST be covered (the denominator).** Walk the source tree(s) and regex/AST
out the population. Override does this THREE ways at once:
```js
function scan(root, isCore) {
  for (const f of walk(root)) {
    const src = stripComments(fs.readFileSync(f, 'utf-8'))
    if (DESKTOP_INFIX.test(f)) { … desktopFiles.push({ file, hasSibling }) }
    for (const key of topLevelSeamKeys(src)) if (!declared.has(key)) declared.set(key, rel(f))
    // + SEAM_USE / USE_OVERRIDE → used, REGISTER → registered
  }
}
```
`declared` (seam keys) is the "must be coverable" set; `enumerateDesktopFiles()` splits desktop
files into `shadows` (must be accounted) vs `exclusive` (legit).

**B. Enumerate what IS covered (the numerator).** From a DIFFERENT committed source:
`registered` (from `registerOverride(…)` in desktop) is "what's covered"; the allow-list
`OVERRIDE_EXCEPTIONS.md` (`parseShadowExceptions`) is "what's explicitly excused".
```js
const shadowExceptions = parseShadowExceptions(exceptionsText)  // committed .md allow-list
```

**C. Compute the missing set (pure, exported for unit test).** The core is a one-liner filter —
"in MUST, not in IS":
```js
export function computeDrift(declared, registered, desktopFiles) {
  return {
    deadOverrides:      [...registered.keys()].filter(k => !declared.has(k)),   // covered-but-not-declared
    orphanDesktopFiles: desktopFiles.filter(d => !d.hasSibling),                // covered-but-no-target
    unregisteredSeams:  [...declared.keys()].filter(k => !registered.has(k)),   // declared-but-uncovered = LEGIT (reported)
  }
}
const unaccountedShadows = desktopShadows.filter(s => !shadowExceptions.has(s)) // MUST \ (IS ∪ allow) = FAIL
```
Note the ASYMMETRY worth copying: "declared-but-uncovered" is legitimate (web-only) → reported,
NOT failed; "covered-but-undeclared" (dead) and "must-cover minus allow-list" (unaccounted) →
FAIL. A completeness gate must decide which direction of the diff is fatal. The pure function is
deliberately extracted + exported so a unit test can drive it (TEST-7).

**D. Fail (and keep the artifact fresh).** Accumulate a `failed` flag across each missing-set
condition, print a SPECIFIC remediation per condition (what to add + WHERE), then combine with
the byte-compare and `process.exit(1)`:
```js
let failed = false
if (deadOverrides.length)      { failed = true; console.error(`override drift: registerOverride() for undeclared seam(s): …`) }
if (unaccountedShadows.length) { failed = true; console.error(`override drift: N RAW whole-file shadow(s) not migrated and not approved:\n…\n  → convert each to a <Seam> … or record an approved "- SHADOW-EXCEPTION: …".`) }
if (orphanDesktopFiles.length) { failed = true; console.error(`override drift: orphaned .desktop file(s) …`) }
if (check) {
  const cur = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf-8') : ''
  if (cur.trim() !== body.trim()) { failed = true; console.error('OVERRIDE_MANIFEST.md is stale — run … and commit.') }
  if (failed) process.exit(1)
  console.log(`override registry OK — …`)
} else {
  fs.writeFileSync(OUT, body)
  if (failed) process.exit(1)   // regenerating does NOT clear a semantic failure
}
```

**Design invariants a new gate MUST copy:**
1. Both MUST-set and IS-set come from **committed product-tree paths**, never `.lifecycle/`
   (B6). The precedent even inlines the reason at the allow-list read and in the exceptions-file
   header: `.lifecycle/` is stripped at merge, so a gate reading it would find zero and fail on
   `main` forever.
2. The allow-list is a committed file with a strict parseable line format
   (`- SHADOW-EXCEPTION: <path> — <reason> [approved: <who/when>]`) requiring a REASON + sign-off.
3. The missing-set computation is a pure exported function → unit-testable.
4. Failures print a specific, actionable remediation (add X to file Y), not just a count.
5. Include a **stale-allow-list** check (overlay does this): an allow-list entry that is now
   covered or whose target vanished is itself a failure, so the excuse list can't rot.
6. `gen` and `check:<name>` are separate npm scripts; `check:<name>` = `--check`, appended to
   `npm run check`. If tsc can carry the completeness gate (a generated union + a hand
   `satisfies Record<Union, Entry>`), prefer that (gallery-coverage / state-matrix) — the
   `.mjs --check` then only keeps the union byte-fresh and tsc does the enforcement.
