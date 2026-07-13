# Survey 05 — Gallery COVERAGE + STATE-MATRIX gates & all allowlists

Worktree: `/data/pbya/ziee/tmp/showcase-seed-wt`. All paths below are under
`src-app/ui/`. Snapshot of what each committed gate enforces TODAY, so the new
per-module "completeness" gate can slot in with a matching escape hatch.

There are **three deterministic (tsc + node) gates** wired into `npm run check`
(package.json:126), plus **one heavy opt-in runtime gate** (`gallery:coverage`,
not in `check`). Each gate follows the same two-part shape:

> **generated union (the denominator, machine-walked) → hand-authored total
> `Record` over that union (compile error if any key is missing) → a node
> `--check` mode that (a) drift-guards the generated file byte-for-byte and (b)
> enforces extra rules.** An allowlist JSON (or an inline `reason`/`skip` field)
> is the escape hatch.

---

## 1. `check:gallery-coverage` — surface-level gate

`node scripts/gen-gallery-coverage.mjs --check` (package.json:171). Gen sibling:
`npm run gen:gallery-coverage` (runs in the openapi/gen flow).

### The two sets that must be equal
- **Denominator (generated):** `src/dev/gallery/galleryCoverage.generated.ts` —
  `GALLERY_SURFACES` = every `.tsx` under `src/modules` **+** `src/components/ui`,
  as `GallerySurface` string ids = path from `src/` minus extension
  (e.g. `modules/llm-provider/components/LlmProviderSettings`,
  `components/ui/kit/button`). **Currently 471 surfaces.**
  - Walk roots (`ROOTS`, gen-gallery-coverage.mjs:29): `src/modules` (skips only
    `module.tsx`) + `src/components/ui` (skips nothing).
  - Excluded dirs: `node_modules`, `dist`, `build`, `.git`, `tests`, `__tests__`.
  - Excluded files by regex: `*.test.tsx`, `*.stories.tsx`, **`*.desktop.tsx`**
    (desktop-only co-located overrides — not web surfaces).
- **Numerator (committed source of truth):** `src/dev/gallery/coverage.ts` —
  `GALLERY_COVERAGE satisfies Record<GallerySurface, Coverage>`. Because it's a
  **total** `Record` over the generated union, a surface with no key is a **tsc
  compile error**, and a stale key (deleted file) is also a tsc error. tsc, not
  the node script, is what forces every surface to be accounted for.

### What `--check` (the node gate) additionally fails on
1. **Staleness:** regenerates the union in memory and byte-compares to the
   on-disk `galleryCoverage.generated.ts`; mismatch → exit 1 ("stale — run
   `npm run gen:gallery-coverage` and commit"). (Same discipline as
   `types_ts_parity`.)
2. **Required-state-set gate:** parses coverage.ts entries with a regex
   (`"id": { kind: '…' … states: […] }`) and, per `REQUIRED` map
   (gen-gallery-coverage.mjs:92, mirrors `REQUIRED_STATES` in coverage.ts:65),
   fails if a surface's declared `states` miss any required state for its `kind`:
   | kind | required states |
   |---|---|
   | `data-page` | loaded, empty, error |
   | `table` | loaded, empty, error |
   | `form` | empty, filled, invalid |
   | `overlay` | open |
   | static / flow / via / nonvisual / pending | none (escape hatches) |
   A miss → exit 1 with `✗ <id> (<kind>) missing: <states>`.
3. **Pending report (NON-fatal):** lists every `kind: 'pending'` surface to
   stdout so tracked TODOs stay visible. Pending does **not** fail the gate.

### How a surface is legitimately EXCLUDED (the `Coverage.kind` escape hatches)
The `SurfaceKind` union (coverage.ts:35) IS the exclusion vocabulary — each kind
with `REQUIRED_STATES = []` opts out of the state gate with an inline `reason`:
- `via` (364 entries) — "rendered inside another covered surface (its page /
  story)"; the dominant escape. e.g. `{ kind: 'via', reason: 'kit-stories' }` or
  `{ kind: 'via', reason: 'rendered within the <x> module page' }`.
- `static` (32) — overlay open-state that needs live context (hub item /
  provider+model / file / prop-driven); allow-listed with reason, verified via
  the e2e interaction suite.
- `nonvisual` (9) — context/provider/listener/types; no visual render.
- `flow` (5) — auth/setup flow (no data grid).
- `pending` (9) — accounted-for TODO with a reason, reported but non-fatal
  (today: the whole `modules/voice/*` cluster + `chat/.../MicButton`, all tagged
  `DRIFT-1`).

**Current kind distribution (471 total):** via 364, data-page 38, static 32,
overlay 14, nonvisual 9, pending 9, flow 5. (`table` and `form` kinds exist in the
type but have 0 entries today — forms are modeled as `overlay` with
`states:['open','filled','invalid']`.)

### How WITHOUT vite / plain Node
The generator is pure `node:fs` recursion (`walk`) + string/regex parsing of
coverage.ts (`parseCoverage`, a single regex at gen-gallery-coverage.mjs:104). No
TS compilation, no vite, no ts-morph, no DOM. It never imports the app. So the
surface set is discovered purely by **file-system walk of `.tsx` paths** — the id
is literally the relative path. (`--scaffold` mode can auto-append missing keys as
`pending('auto-scaffold — needs review')` at the `// <<< scaffold-insert >>>`
marker in coverage.ts:551, but that's opt-in and not part of gen/check.)

---

## 2. `check:state-matrix` — state-granularity gate (Part 1 of the exhaustive-state mechanism)

`node scripts/gen-state-matrix.mjs --check` (package.json:175). This is a
**different, finer** gate than #1: #1 asks "does each surface have an entry with
its required states"; #2 asks "does each mechanically-extracted renderable STATE
have a delivery/excuse".

### How it derives required states (AST, not judgment)
Uses **ts-morph** over the tsconfig (`gen-state-matrix.mjs:46`, needs the TS
project — heavier than #1, but still node, no vite/DOM). Globs `modules/**/*.tsx`
+ `components/ui/**/*.tsx`. Per surface it extracts SIGNALS:
- **(a) conditional renders** that render JSX — ternary `c ? <A> : <B>`, logical
  `c && <JSX>`, early `if (c) return <JSX|null>`. Each condition is classified by
  the identifiers in it (`CLASSIFIERS`, gen-state-matrix.mjs:70; order matters:
  **error → loading → empty**, residual = `branch`):
  - `error` ← `error|isError|hasError|failed|loadError|fetchError`
  - `loading` ← `loading|isLoading|isPending|isFetching|isInitializing|pending|spinner|skeleton`
  - `empty` ← `.length === 0`, `!x.length`, `isEmpty`, `no[A-Z]…`, `.size === 0`, …
  - `branch` ← anything else (a variant fork — tracked, NOT gated by tsc).
- **(b) overlay open-triggers** — a JSX tag matching
  `(AlertDialog|Dialog|Drawer|Sheet|Popover|DropdownMenu|Dropdown|Modal|Confirm|HoverCard|ContextMenu)$`
  carrying a controlled `open|visible|defaultOpen|isOpen` prop → signal `overlay`.
- **(c) panel + slot registrations** — `registerPanelRenderer('type')` → signal
  `panel` (+ recorded in `PANEL_RENDERERS`); `registerSlot(...)` and object-literal
  `slots: { <KNOWN_SLOTS> }` → recorded in `SLOT_REGISTRATIONS` (discoverability
  map; not itself a required state).

Signal kind → required gallery state (`KIND_TO_STATE`, gen-state-matrix.mjs:227):
`loading→delayed`, `error→error`, `empty→empty`, `overlay→open`, `panel→panel-open`.
`branch` demands NO named state (proven only by Part 2 runtime coverage).

### The two sets that must be equal
- **Denominator (generated):** `src/dev/gallery/stateMatrix.generated.ts` —
  exports `STATE_MATRIX` (per-surface signals + requiredStates), `PANEL_RENDERERS`,
  `SLOT_REGISTRATIONS`, and the key type
  **`RequiredState` = union of `"<surface>:<state>"`** (one member per named
  required state). Also `STATE_MATRIX.md` (human review artifact). Only surfaces
  carrying ≥1 signal appear; `module.tsx` and `.desktop.tsx` are excluded (slots
  from module.tsx are still harvested, just no surface row).
- **Numerator (committed):** `src/dev/gallery/stateCoverage.ts` —
  `STATE_COVERAGE satisfies Record<RequiredState, StateCoverageEntry>`. Total
  Record over the generated union → a newly-extracted state with no entry is a
  **tsc compile error**; a stale key is a tsc error. **374 keys today, 292 of them
  `skip:true`.**

### What `--check` fails on
1. **Staleness:** byte-compares BOTH `stateMatrix.generated.ts` AND
   `STATE_MATRIX.md` to freshly-rendered bodies; stale → exit 1 ("a new
   conditional render was added… regen adds its RequiredState key; then map it in
   stateCoverage.ts").
2. **Missing key report:** computes `requiredStateKeys − stateCoverage keys`; if
   any residual → exit 1 (prints up to 20). (This is belt-and-suspenders — tsc
   already fails on the same gap via the `satisfies` Record.)

### How a NEW conditional-render state is forced to appear
Add a new `if (error) return …` (or overlay `open`, etc.) → `gen:state-matrix`
regenerates a stale file → `check:state-matrix` fails on staleness → dev regens →
a new `RequiredState` member `"<surface>:error"` appears → **tsc fails** on
stateCoverage.ts until that key gets either a delivery `{ via }` or an
allow-listed `{ skip:true, reason }`. This is the mechanism that "replaces agent
judgment with the code" (EXHAUSTIVE_STATE.md).

### The escape hatch (inline, in stateCoverage.ts — NOT a separate JSON)
`StateCoverageEntry = StateDelivered | AllowlistedGap`:
- **Delivered:** `{ via: string }` where via ∈ `'page-state-mode'` (browsed in a
  data-mode via `?surface=&state=`), `'overlay'` (an overlays.tsx entry),
  `'deep:<slug>'` (a deepStates.tsx entry), `'interaction:<slug>'` (an
  interactions.ts recipe drives a post-mount action).
- **Allow-listed gap:** `{ skip: true, reason: string }` — excused in code. Used
  heavily for `via` components whose state only renders inside their parent page
  (branch proven by Part 2 runtime coverage), and for genuinely-undrivable
  transient states. `--scaffold` (gen-state-matrix.mjs:481) auto-fills missing
  keys with a SEMANTICALLY-CORRECT default from the surface's `coverage.ts` kind:
  data-page/table + error|empty|delayed → `{ via: 'page-state-mode' }`; overlay +
  open → `{ via: 'overlay' }`; else `{ skip:true, reason: '<kind> surface —
  rendered within its page; …branch proven by Part 2…' }`.

---

## 3. `check:overlay-registry` — overlay-render gate

`node scripts/gen-overlay-registry.mjs --check` (package.json:173). Gen sibling
`gen:overlay-registry`. Closes: overlays were never rendered OPEN in the gallery,
so no geometry/affordance/runtime/vision audit ever saw them.

### What it walks / classifies
Pure `node:fs` walk (no vite) of `src/modules` (skips `module.tsx`) +
`src/components/ui`, excluding `*.test.tsx`/`*.stories.tsx` (note: `.desktop.tsx`
is NOT excluded here, unlike #1/#2). For each file it:
- Reads which overlay PRIMITIVES (`Dialog, Drawer, Sheet, Modal, Popover,
  AlertDialog, Confirm, Popconfirm`) are imported **from the kit**
  (`@/components/ui[/…]` or the layout `Drawer`) — a same-named local import does
  NOT trip the gate.
- Scans JSX occurrences and classifies each surface:
  - **`host`** (`controlled`) — the primitive has a controlled open prop
    (`open=` / `open` shorthand / `isOpen` / `visible=`). Its whole job is to
    render an overlay a store/prop drives → it **MUST be rendered open** in the
    gallery, or allow-listed.
  - **`trigger`** (self-opening) — a Confirm/Popover/menu wrapping a trigger child
    that opens on interaction (no open prop) → needs a `triggers` allow-list entry
    (or its parent wired + an interaction recipe).
- Emits `src/dev/gallery/overlay-registry.generated.json`
  (counts + `hosts` + `triggers`).

### What "wired open" means
`wiredSurfaces()` regexes `surface: '…'` occurrences out of
`src/dev/gallery/overlays.tsx` (the `OVERLAY_ENTRIES` list). A host counts as
delivered iff its id appears there.

### What `--check` fails on
1. **Staleness:** byte-compares regenerated registry JSON to disk.
2. **Missing hosts:** any `host` that is neither wired in overlays.tsx nor in
   allowlist `hosts` → exit 1 ("N overlay HOST(s) never rendered open… add an
   OVERLAY_ENTRIES entry OR an allow-list reason").
3. **Missing triggers:** any `trigger` not wired and not in allowlist `triggers`
   → exit 1.
4. **Stale allow-list entries:** an allow-list key that is now wired OR whose
   surface no longer exists → exit 1 ("keep the list honest"). This is the notable
   extra: the overlay gate actively GC-checks its own allowlist, which #1/#2 do not
   do for their inline reasons.

`--list` prints a `[wired|allowlisted|MISSING]` table per host/trigger.

---

## 4. The allowlist pattern (system-wide) — the escape-hatch schema per gate

Two shapes are used across the gallery. **Schema = a keyed map/list where the key
identifies the excused thing and the value is a human-readable `reason`.** No
severity, no expiry — an entry is an explicitly-accepted gap, reviewed in code
review. For a new completeness gate, the closest precedents are the overlay JSON
(external file, GC-checked) and the coverage-allowlist JSON (external file,
line-keyed).

| Allowlist file | Gate it feeds | Key schema | Value | Notes |
|---|---|---|---|---|
| `coverage-allowlist.json` | Part 2 runtime branch coverage (`gallery-coverage.mjs`, NOT in `check`) | `"<surface-file>:<line>"` (opt `:<arm>`) | reason string | flat object; `__doc__` header states the policy (interaction-gated arms must get a recipe, not an allow-list). |
| `overlay-allowlist.json` | `check:overlay-registry` | `{ "hosts": {<surface-id>: reason}, "triggers": {<surface-id>: reason} }` | reason string | two buckets; `$comment` header; STALE ENTRIES FAIL the gate. |
| `geometry-allowlist.json` | Layer-1 geometry audit (`gallery-geometry-audit.mjs --gate`) | `{ entries: [{ class, surface, selector?, viewport?, reason }] }` | reason (finding still REPORTED, just not gating) | class-based (A1/B1/B3/I4/L1/L5); `surface:'*'` = any. Only HIGH classes gate. |
| `icon-action-allowlist.json` | `lint:icon-action` (taxonomy C11) | `{ entries: [{ testid?, file?, action?, reason }] }` | reason | currently EMPTY. Substring match on any provided field. |
| `native-scroll-allowlist.json` | `lint:native-scroll` (J8, advisory) | `{ entries: [{ file, reason }] }` | reason | currently EMPTY; grandfather list. |
| inline `coverage.ts` `reason` | `check:gallery-coverage` state gate | the `Coverage.kind` + `reason` field | reason | NOT a separate file — the `kind` (via/static/nonvisual/flow/pending) IS the exclusion, `reason` documents it. |
| inline `stateCoverage.ts` `{skip,reason}` | `check:state-matrix` | the `"<surface>:<state>"` key + `skip:true` | reason | NOT a separate file — excused in the same Record. |

**Two allowlist styles in play:** (a) **inline in the coverage Record** (#1 uses
`kind`+`reason`; #2 uses `{skip,reason}`) — tsc-enforced totality guarantees no
silent gap; (b) **external JSON keyed by id/line/class** (#3 overlays, Part-2
coverage, geometry, lint gates). The overlay JSON is the best template for a new
"module has a surface but no seed" gate because it: (i) is a plain JSON with a
`hosts`/`triggers`-style bucketed `{id: reason}` map, (ii) has a doc header
stating the policy, and (iii) is **actively GC-checked for stale/now-satisfied
entries** — the honesty property a completeness gate wants.

---

## 5. Existing notion of "module has a surface but no seed" + intended end-state

**There is a partial, per-surface (not per-module) encoding today, but no
first-class "module completeness" gate.**

- **`kind: 'pending'`** in coverage.ts is the closest existing "surface exists but
  is not seeded/rendered" marker: "tracked TODO: accounted for, not yet given a
  visual entry." It is **reported but non-fatal** by `check:gallery-coverage`. The
  entire live pending set (9 entries) is the `modules/voice/*` admin cluster +
  `chat/.../MicButton`, each reasoned `gallery cell deferred (DRIFT-1); flow
  covered by the 14-voice e2e specs`. So "surface but no seed" today = a `pending`
  reason string pointing at an e2e spec instead of a gallery cell.
- **`kind: 'static'`** (32 entries) is the adjacent concept for OVERLAYS whose
  open-state "needs a live open-trigger context" — genuinely surfaceless in the
  mount-only gallery, "verified via the e2e interaction suite". Same escape-hatch
  spirit (reason + e2e pointer) but asserts it IS covered elsewhere, vs `pending`
  which admits a deferred gap.
- The overlay gate's allowlist entries almost universally end with **"Tracked for
  an interaction recipe"** / **"future interaction recipe"** — i.e. an explicit
  "this overlay is not yet seeded open; here's why and what would fix it."
- **`stateCoverage.ts` `{skip, reason}`** (292 entries) is the state-level "no seed
  for this branch" marker, with the reason distinguishing "proven by Part 2 runtime
  coverage" (fine) from "not yet exercised by any spec (DRIFT-1)" (an honest gap).

### Intended end-state per the design docs
- **`SEEDED_GALLERY_PLAN.md`**: the target is EVERY page + EVERY store's populated
  state + EVERY module component rendered with realistic seeded data, with
  per-surface MULTIPLE named states (loaded/empty/error/filled/invalid/open). "The
  41 `pending` surfaces are interaction-only overlays… Each needs a gallery entry
  that renders it in its OPEN state with seeded data." The plan's declared
  end-state = **0 pending; 100% delivered or reviewed allow-list.**
- **`COVERAGE.md`** (STALE — says 407 surfaces / pending 0 / via 321 / static 27;
  reality is 471 / pending 9 / via 364 / static 32) claims "100% of surfaces
  accounted." Treat its numbers as out-of-date; coverage.ts + the generated union
  are the truth.
- **`EXHAUSTIVE_STATE.md`**: three mechanical layers (Part 1 tsc state gate, Part 2
  runtime branch coverage, Part 3 deep-state cassettes) so **every renderable state
  is either rendered by a gallery entry or explicitly excused** — judgment replaced
  by machinery.
- **`README.md`**: interaction recipes (`interactions.ts`,
  `?surface=<slug>&interact=<name>`) are the sanctioned way to convert an
  interaction-gated `{skip}` into a delivered `{ via: 'interaction:<slug>' }` — the
  intended migration path OUT of the pending/skip allowlists.

**Gap for the new gate:** nothing today enumerates per-MODULE whether a module has
≥1 genuine seeded surface (a page/overlay/deep/seeded entry rendering real data) vs
being wholly covered by `via`/`pending`/`static` reasons. The surface enumeration
lives in `surfaces.ts::listAllSurfaces()` (four classes: `pages` read from browse
DOM, `overlays`/`deep`/`seeded` static lists) — the natural place to hang a
completeness check is a map of module → does it contribute any pages/overlays/deep/
seeded slug, mirroring the overlay gate's JSON-allowlist shape (`{module: reason}`,
GC-checked) for genuinely-surfaceless modules (e.g. `router`, `notification`
listeners, pure chat-extension registrations).

---

## Quick reference — files & wiring

- Gates in `npm run check` (package.json:126, in order):
  `check:gallery-coverage`, `check:gallery-crawl`, `gallery:check-fixtures`,
  `check:state-matrix`, `check:overlay-registry`, `check:override-registry`.
- NOT in check (heavy, opt-in): `gallery:coverage` / `gallery:coverage:gate`
  (`gallery-coverage.mjs`, Part 2 runtime branch coverage → `UNCOVERED_STATES.md`).
- Denominator generators: `gen-gallery-coverage.mjs` (fs walk),
  `gen-state-matrix.mjs` (ts-morph), `gen-overlay-registry.mjs` (fs walk).
- Committed Records: `coverage.ts` (471), `stateCoverage.ts` (374),
  `overlays.tsx` (wired hosts) + `overlay-allowlist.json`.
- Generated (do-not-edit): `galleryCoverage.generated.ts`,
  `stateMatrix.generated.ts` + `STATE_MATRIX.md`,
  `overlay-registry.generated.json`.
- Surface enumeration single-source: `surfaces.ts::listAllSurfaces()` →
  `window.__GALLERY_LIST_ALL_SURFACES__` (classes: pages/overlays/deep/seeded +
  interactions).
