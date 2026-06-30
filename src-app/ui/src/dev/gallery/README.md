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

## Documented pre-existing kit findings (the system already caught these)

Real defects surfaced by the layers, recorded in `axe-baseline.ts` /
`layout-baseline.ts` so the gate fails only on NEW issues (this branch builds the
harness; fixing the kit is separate). Delete an entry when its kit issue is fixed
— the gate then enforces it.

1. **Status/tone color contrast** (`color-contrast`) — kit `Tag`/`Alert`/`Text`
   status tones use hardcoded palette hues, not dark-aware AA tokens; fail WCAG AA
   in dark mode.
2. **`Menu` list markup** (`list`) — `<ul>` directly contains `<button>`s, not `<li>`.
3. **Long-content containment** (`childOverflow`/`textTruncation` on `stress-*`) —
   `Tag`, `Select` trigger, `Card` title, `Menu`, `Descriptions` don't contain
   long unbroken content (missing `break-word`/`min-w-0`/ellipsis) → overflow.
4. **`Table` scroll region not keyboard-focusable** (`scrollable-region-focusable`)
   — the overflow-auto viewport needs `tabindex=0`.
