# Visual / aesthetic testing — the component gallery + three layers

Automated catch for "the button is too big / spans the row / misaligned / padding
looks wrong / fails contrast in dark mode" — across themes and accent colors,
**before a human looks**. Implements
`.claude/audit/shadcn-migration/VISUAL_TESTING_GUIDE.md`.

It's three layers over one shared canvas — the **gallery**:

| Layer | What | Where | In CI? |
|---|---|---|---|
| Gallery | dev-only canvas: every kit component × variant/state/tone/size + composite scenes, URL-driven by `?theme=&accent=` | `src/dev/gallery/` | n/a (dev route) |
| **A** | deterministic layout invariants + axe a11y (no baseline) | `tests/e2e/helpers/layout.ts`, `tests/e2e/visual/layout.spec.ts` | yes, every run |
| **B** | visual-regression snapshots over the matrix (3 viewports × 2 themes × **all 8 accents** = 48 cells; subset via `VISUAL_ACCENTS`) | `tests/e2e/visual/gallery.spec.ts` | yes, needs blessed baseline |
| **C** | vision-model aesthetic judge (delta-gated, on-demand) | `scripts/visual-judge.mjs` | **no** — off the test loop |

## The gallery

- **In-app route** `/dev/gallery` (gated on `import.meta.env.DEV` — never ships)
  for manual review inside the real app shell. Module: `src/modules/dev-gallery/`.
- **Standalone, backend-free** entry `/dev-gallery.html` — registers only the
  `ConfigClient` store and renders the gallery under the real `ThemeProvider`.
  This is what the Playwright layers drive (no Postgres, no `cargo run`).
- URL matrix: `/dev-gallery.html?theme=dark&accent=teal` re-renders the WHOLE
  gallery under that combo. A control bar (theme + accent Select) does the same
  for eyeballing.
- Add a component: drop a `GalleryStory` into the matching `stories/*.story.tsx`
  (use `components/ui/KIT_MANIFEST.md` as the prop checklist) and export it from
  `stories/index.ts`. Section testid = `gallery-section-<id>`; case testid =
  `gallery-case-<id>-<key>`. These ids are computed, so they stay out of the
  app's typed testid registry (the generator excludes `dev/`).

## Running

```bash
# Layer A + B (boots the gallery Vite server automatically, no backend):
npm run test:visual

# Resolve specs only (the gate):
npm run test:visual:list

# Bless / re-bless Layer B baselines (per environment — see below):
npm run test:visual:update

# Layer C (off the test path; needs ANTHROPIC_API_KEY, or --dry-run):
npm run visual:judge -- --only-changed            # delta-gated
node scripts/visual-judge.mjs --dry-run            # wiring check
```

### Layer B baselines are environment-specific

Snapshot PNGs depend on the OS/font rendering of the machine that blessed them,
so they are **git-ignored** (`tests/e2e/visual/**/*-snapshots/`). Bless once on
your CI runner image (or a pinned container) with `test:visual:update`; commit
those if your CI image is stable, or re-bless in CI. Never commit a dev laptop's
PNGs — they'll false-fail everywhere else.

## Layer A — what it enforces (`assertLayoutSane`)

No horizontal page scroll · no child overflows its (non-clipping) parent · no
in-flow sibling overlap · spacing/gap/radius on the 2px/`--radius` scale ·
non-block buttons don't span their container · touch targets ≥ 24px (standalone
controls only) · no silent text truncation. Plus `@axe-core/playwright` for
WCAG 2A/2AA. Tunable via `LayoutSaneOptions` (grid, radii, per-check toggles).
The helper is reusable on real (backend-ful) pages too.

## Extra coverage to catch more bugs

Beyond the per-component variant grid, the gallery includes the highest-yield
bug-finding patterns from visual-testing prior art (Chromatic / EightShapes):

- **Content-stress sections** (`stress.story.tsx`, `stress-*`): every torture
  input — long UNBROKEN tokens, i18n-expanded compounds, long prose, empty +
  loading states, zero-data tables, huge numbers — inside deliberately narrow
  containers, so overflow/truncation/wrap/clipping failures surface.
- **RTL pass**: `?dir=rtl` flips the whole gallery; Layer A runs the invariants
  under RTL (mirroring/alignment/logical-property bugs).
- **Breakpoints**: mobile/tablet/desktop (already in the matrix).
- **Overlay OPEN states** (`overlays.spec.ts`): opens Dialog/Sheet/Confirm/
  Dropdown/Select/Popover and snapshots + layout-checks the actual portal content
  (per-section shots never captured these).
- **Interactive states** (`states.spec.ts`): drives real `.hover()`/`.focus()`
  on key controls, asserts the focus ring doesn't overflow, and snapshots them.
- **Full Button matrix** (variant × size + per-variant disabled/loading),
  **Tabs editable** (add/close cards), **Tree checkable**, and the 8 components
  the audit found missing (Space, Layout, ScrollArea, Image, Upload, Attachment,
  SidebarTrigger, FormList).

## Kit defects the system caught — now FIXED + enforced

The layers surfaced real pre-existing kit defects; all were FIXED in the kit and
the gate now enforces them with **empty baselines** (`axe-baseline.ts` /
`layout-baseline.ts`). Each fix was verified by removing its baseline and
re-running until green:

1. **Status/tone color contrast** — `Tag`/`Alert`/`Text` status tones used raw
   palette hues → failed WCAG AA in dark mode. Remapped to the dark-aware
   semantic tokens (`text-success`/`-warning`/`-info` + `destructive`). Verified:
   axe green in both themes with no baseline.
2. **`Menu` list markup** — a `<li role="separator">` made the `<ul>` contain a
   non-listitem child. Moved the separator to an inner element.
3. **`Table` scroll region not keyboard-focusable** — added `tabIndex=0` to the
   overflow-auto viewport (shadcn table).
4. **Long-content containment** — `Tag`/`Card` title/`Menu` item/`Descriptions`
   value now contain long unbroken content (`min-w-0` + `overflow-wrap`/`truncate`).
   (The "`Select` clips without ellipsis" report was a CHECKER false-negative —
   `line-clamp` wasn't recognized as an ellipsis affordance; fixed in `layout.ts`.)

Expanding coverage (overlays, missing components, hover/focus, Button matrix)
caught **more** real kit defects — fixed: Upload `nested-interactive` (file input
nested in the role=button dropzone → now a sibling), Tabs editable add-button
inside `role=tablist`. Two remain **baselined** (documented, element-keyed) as
harder fixes: editable/closable Tabs `aria-required-children` (needs the ARIA-APG
deletable-tabs pattern) and ScrollArea's third-party scroll-viewport `tabindex`.

The baselines remain as the documented mechanism for any FUTURE finding — keyed
as narrowly as possible (axe by `rule × section [× target]`, layout by
`section × check × testid`) so a baseline can't mask a new violation elsewhere.
