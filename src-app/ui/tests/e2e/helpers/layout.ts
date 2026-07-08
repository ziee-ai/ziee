/**
 * Layer A — deterministic layout invariants (`assertLayoutSane`).
 *
 * Runs against the component gallery's sections AND key real pages. Unlike the
 * screenshot layer (B), these need NO blessed baseline — they encode rules that
 * are bugs by definition regardless of theme/accent:
 *   - no horizontal page scroll
 *   - no child overflows its parent box / clipped off-viewport
 *   - no overlap between siblings that should be laid out in flow
 *   - spacing (padding / margin / gap / border-radius) on the 4px / --radius scale
 *   - a non-block button isn't wider than its container ("spans the whole row")
 *   - elements that should share an edge are aligned (equal x or y)
 *   - touch targets meet a minimum hit size
 *   - no unintended text truncation (scrollWidth > clientWidth with no ellipsis)
 *
 * Implementation: `boundingBox()` + a single `getComputedStyle` sweep in the
 * page context. Pair with `@axe-core/playwright` at the call site for a11y.
 *
 * See `.claude/audit/shadcn-migration/VISUAL_TESTING_GUIDE.md` §2.
 */
import { expect, type Locator, type Page } from '@playwright/test'

export interface LayoutSaneOptions {
  /** Allowed spacing grid in px. Computed paddings/margins/gaps must be multiples. */
  grid?: number
  /** Allowed border-radius values in px (the `--radius` ramp). 0 always allowed. */
  radii?: number[]
  /** Minimum interactive hit size in px (WCAG 2.5.8 = 24; AAA 2.5.5 = 44). */
  minTouchTarget?: number
  /** Sub-pixel tolerance for alignment / overflow comparisons. */
  tolerance?: number
  /** Which invariant groups to run (default: all). */
  checks?: Partial<Record<LayoutCheck, boolean>>
  /** Extra CSS selectors to exclude (e.g. known-irregular third-party widgets). */
  ignoreSelectors?: string[]
  /** Return the violations instead of asserting (lets the caller filter against
   *  a documented layout baseline before failing). Default false → asserts. */
  collect?: boolean
}

export type LayoutCheck =
  | 'horizontalScroll'
  | 'childOverflow'
  | 'siblingOverlap'
  | 'spacingScale'
  | 'buttonWidth'
  | 'touchTarget'
  | 'textTruncation'

const DEFAULTS = {
  // Tailwind's spacing ramp is 2px-based (0.5=2, 1=4, 1.5=6, 2.5=10, …), so 2px
  // is the real "on-scale" invariant; 4px would false-positive on legitimate
  // half-step utilities the kit uses.
  grid: 2,
  // The --radius ramp in index.css plus pill/circle (9999) and common half-steps.
  radii: [0, 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 9999],
  minTouchTarget: 24,
  // Geometric tolerance for OVERFLOW / ALIGNMENT / TOUCH comparisons (sub-pixel
  // layout jitter). NOT used for on-scale snapping.
  tolerance: 1.5,
  // On-scale tolerance for spacing/radius snapping. MUST be < grid/2 or the
  // check is mathematically dead (every value lands within tol of a grid
  // multiple). 0.5 catches a genuine off-grid 7px/13px while absorbing rem
  // rounding.
  scaleTolerance: 0.5,
} as const

export interface LayoutViolation {
  check: LayoutCheck
  testid: string | null
  message: string
}

/** One serializable record per candidate element, gathered in the page context. */
interface ElementProbe {
  /** Stable index within this probe pass. */
  index: number
  /** Index of the nearest probed ancestor (parent), or -1 for the scope root. */
  parentIndex: number
  /** Number of element children the parent has (for "sole child" heuristics). */
  parentChildCount: number
  testid: string | null
  tag: string
  role: string | null
  rect: { x: number; y: number; w: number; h: number }
  parentRect: { x: number; y: number; w: number; h: number } | null
  styles: {
    paddings: number[]
    margins: number[]
    gap: number[]
    radius: number[]
    display: string
    position: string
    width: string
    overflowX: string
    whiteSpace: string
    textOverflow: string
    lineClamp: string
  }
  scrollWidth: number
  clientWidth: number
  isButton: boolean
  isBlock: boolean
  hasTextChild: boolean
  /** True for in-field affix controls (a button sharing a parent with an
   *  input/textarea: clear ×, password eye, etc.) — exempt from touch-target
   *  minimums, which target standalone controls. */
  insideControl: boolean
  /** Parent's computed overflow-x — a clipping parent (hidden/auto/scroll/clip)
   *  intentionally manages child overflow (progress fills, masked content). */
  parentOverflowX: string
  /** Parent's computed overflow-y (for vertical overflow detection). */
  parentOverflowY: string
  /** Has a CSS transform (translate/scale) — its layout box and visual box
   *  differ by design (switch thumbs, translated progress fills), so it's
   *  exempt from box-overflow checks. */
  transformed: boolean
}

/**
 * Collect a layout probe for every visible element under `scope`, in ONE page
 * evaluate (cheap + consistent snapshot). `scope` is a CSS selector resolved in
 * the page; pass the section's testid selector from the caller.
 */
async function probe(
  page: Page,
  scopeSelector: string,
  ignoreSelectors: string[],
): Promise<ElementProbe[]> {
  return page.evaluate(
    ({ scopeSelector, ignoreSelectors }) => {
      const root = document.querySelector(scopeSelector)
      if (!root) return []
      const px = (v: string) =>
        v
          .split(' ')
          .map(s => parseFloat(s))
          .filter(n => !Number.isNaN(n))
      const ignored = (el: Element) =>
        ignoreSelectors.some(sel => el.closest(sel) != null)

      const out: ElementProbe[] = []
      const all = [root, ...Array.from(root.querySelectorAll('*'))]
      // Map each probed element to its index so children can reference their
      // parent by IDENTITY (not bounding-box coords — a single child that
      // stretches to its parent's box would otherwise be mis-grouped as a sibling).
      const indexOf = new Map<Element, number>()
      all.forEach((el, i) => indexOf.set(el, i))
      for (const el of all) {
        if (!(el instanceof HTMLElement)) continue
        if (ignored(el)) continue
        const cs = getComputedStyle(el)
        if (cs.display === 'none' || cs.visibility === 'hidden') continue
        // <colgroup>/<col> are non-painting table-structure elements: their layout
        // box spans the whole column region (so it geometrically "overlaps"
        // thead/tbody by design) yet carries no visual design target. Measuring
        // them yields only false positives (siblingOverlap/childOverflow), so
        // exclude them from every invariant — same intent as the sr-only skips below.
        if (cs.display === 'table-column' || cs.display === 'table-column-group')
          continue
        const r = el.getBoundingClientRect()
        if (r.width === 0 && r.height === 0) continue
        // Skip visually-hidden / hairline elements: sr-only form mirrors (a 1px
        // native <select>/<input> kept for form binding), clipped a11y text, and
        // 1px separators. They carry UA-default spacing that isn't a design
        // target and aren't visible, so layout invariants don't apply.
        const clipped =
          cs.clip === 'rect(0px, 0px, 0px, 0px)' ||
          cs.clipPath === 'inset(50%)' ||
          cs.clipPath === 'inset(100%)'
        if (r.width <= 1 || r.height <= 1 || clipped) continue
        const parent = el.parentElement
        const pr = parent ? parent.getBoundingClientRect() : null
        const index = indexOf.get(el)!
        const parentIndex =
          parent && indexOf.has(parent) ? indexOf.get(parent)! : -1
        const parentChildCount = parent ? parent.childElementCount : 0
        const parentCS = parent ? getComputedStyle(parent) : null
        const parentOverflowX = parentCS ? parentCS.overflowX : 'visible'
        const parentOverflowY = parentCS ? parentCS.overflowY : 'visible'
        const tag = el.tagName.toLowerCase()
        const role = el.getAttribute('role')
        const isButton =
          tag === 'button' || role === 'button' || tag === 'a'
        const hasTextChild = Array.from(el.childNodes).some(
          n => n.nodeType === Node.TEXT_NODE && n.textContent!.trim().length > 0,
        )
        // In-field affix? Climb a few levels: an affix button lives in a small
        // field wrapper whose subtree also holds the input/textarea. Bounded
        // climb + size guard so a standalone button in a wide toolbar (whose
        // section happens to contain inputs elsewhere) isn't mis-flagged.
        let insideControl = false
        if (isButton) {
          const controlSel =
            'input, textarea, select, [role="textbox"], [role="combobox"], [role="listbox"]'
          let anc: HTMLElement | null = el.parentElement
          for (let depth = 0; depth < 4 && anc; depth++) {
            if (
              anc.getBoundingClientRect().width < 480 &&
              // the control role may be ON the ancestor (combobox wrapper) or in
              // its subtree (an input next to the affix).
              (anc.matches(controlSel) || anc.querySelector(controlSel) != null)
            ) {
              insideControl = true
              break
            }
            anc = anc.parentElement
          }
        }
        out.push({
          index,
          parentIndex,
          parentChildCount,
          testid: el.getAttribute('data-testid'),
          tag,
          role,
          rect: { x: r.x, y: r.y, w: r.width, h: r.height },
          parentRect: pr
            ? { x: pr.x, y: pr.y, w: pr.width, h: pr.height }
            : null,
          styles: {
            paddings: [
              cs.paddingTop,
              cs.paddingRight,
              cs.paddingBottom,
              cs.paddingLeft,
            ].flatMap(px),
            margins: [
              cs.marginTop,
              cs.marginRight,
              cs.marginBottom,
              cs.marginLeft,
            ].flatMap(px),
            gap: px(cs.gap || '0'),
            radius: [
              cs.borderTopLeftRadius,
              cs.borderTopRightRadius,
              cs.borderBottomRightRadius,
              cs.borderBottomLeftRadius,
            ].flatMap(px),
            display: cs.display,
            position: cs.position,
            width: cs.width,
            overflowX: cs.overflowX,
            whiteSpace: cs.whiteSpace,
            textOverflow: cs.textOverflow,
            // `-webkit-line-clamp` (Tailwind `line-clamp-N` / `truncate` variants)
            // is a valid ellipsis affordance the bare text-overflow check misses.
            lineClamp: cs.webkitLineClamp || cs.getPropertyValue('-webkit-line-clamp') || 'none',
          },
          scrollWidth: el.scrollWidth,
          clientWidth: el.clientWidth,
          isButton,
          isBlock: cs.display.includes('block') || cs.display === 'flex',
          hasTextChild,
          insideControl,
          parentOverflowX,
          parentOverflowY,
          // Tailwind v4 emits `translate-x-*` via the `translate` CSS property
          // (not `transform`), so check both (+ scale) to catch translated
          // decorative elements like the switch thumb.
          transformed:
            cs.transform !== 'none' ||
            cs.translate !== 'none' ||
            cs.scale !== 'none',
        })
      }
      return out
    },
    { scopeSelector, ignoreSelectors },
  )
}

const nearestTestid = (p: ElementProbe) => p.testid

function onScale(value: number, grid: number, tol: number): boolean {
  if (value <= tol) return true
  const r = value % grid
  return r <= tol || grid - r <= tol
}

/**
 * Assert the layout invariants over a scope. `scope` is a Playwright Locator
 * (typically a `gallery-section-*` or a page region); its first match's CSS
 * path is used to re-query in the page context.
 *
 * Throws (via `expect`) with a readable list of violations if any are found.
 */
export async function assertLayoutSane(
  scope: Locator,
  options: LayoutSaneOptions = {},
): Promise<LayoutViolation[]> {
  const opts = { ...DEFAULTS, ...options }
  const checks = options.checks ?? {}
  const enabled = (c: LayoutCheck) => checks[c] !== false
  const page = scope.page()

  // Resolve a selector that uniquely identifies THIS scope element so probe()
  // re-queries the right node. Prefer its data-testid; otherwise stamp a unique
  // marker attribute (a bare tagName would match the first such tag in the
  // document — e.g. a role-resolved overlay div resolving to gallery-root).
  const scopeSelector = await scope.evaluate(el => {
    const tid = el.getAttribute('data-testid')
    if (tid) return `[data-testid="${CSS.escape(tid)}"]`
    const existing = el.getAttribute('data-ls-scope')
    const marker = existing ?? `s${Math.random().toString(36).slice(2)}`
    if (!existing) el.setAttribute('data-ls-scope', marker)
    return `[data-ls-scope="${marker}"]`
  })

  const probes = await probe(page, scopeSelector, opts.ignoreSelectors ?? [])
  const violations: LayoutViolation[] = []
  const tol = opts.tolerance

  // 0. No horizontal page scroll (document-level; checked once per call).
  if (enabled('horizontalScroll')) {
    const overflow = await page.evaluate(() => {
      const el = document.scrollingElement || document.documentElement
      return el.scrollWidth - el.clientWidth
    })
    if (overflow > tol) {
      violations.push({
        check: 'horizontalScroll',
        testid: null,
        message: `document scrolls horizontally by ${overflow.toFixed(1)}px`,
      })
    }
  }

  for (const p of probes) {
    // 1. Child overflow: an in-flow element must not extend beyond its parent's
    //    box (unless the parent explicitly scrolls/clips on that axis).
    //    Absolutely/fixed-positioned elements (corner badges, popovers, tooltips)
    //    are placed deliberately and may sit outside the parent — skip them.
    const positioned =
      p.styles.position === 'absolute' || p.styles.position === 'fixed'
    // A `display:contents` parent (e.g. <fieldset class="contents">) has no
    // layout box — its rect is 0×0, so overflow against it is meaningless.
    const parentHasBox = !!p.parentRect && p.parentRect.w > 0
    // Pure `inline` elements have a line-box model: vertical padding/border
    // bleeds OUTSIDE the line without expanding the parent (a CSS quirk, not an
    // overflow bug). Box-overflow only applies to block/flex/grid/inline-block.
    const isInline = p.styles.display === 'inline'
    if (
      enabled('childOverflow') &&
      p.parentRect &&
      parentHasBox &&
      !positioned &&
      !p.transformed &&
      !isInline
    ) {
      // Only auto/scroll parents are a clean exemption (they're meant to scroll).
      // hidden/clip parents still CLIP overflowing content — a real bug — so they
      // are NOT exempted here.
      const xScrolls =
        p.parentOverflowX === 'auto' || p.parentOverflowX === 'scroll'
      const yScrolls =
        p.parentOverflowY === 'auto' || p.parentOverflowY === 'scroll'
      // A negative margin deliberately pulls the border-box |margin| past the
      // (flex/inline) parent's box that sized to the margin-box — e.g. a
      // `-mx-1 px-1` hover-padding button whose bg bleeds 4px but stays inside the
      // real cell. Discount that slack per side so the pattern isn't mis-flagged
      // as an overflow bug (mirrors the negative-margin exemption in
      // detectSiblingOverlap). margins = [top, right, bottom, left].
      const [mTop, mRight, mBottom, mLeft] = [
        p.styles.margins[0] ?? 0,
        p.styles.margins[1] ?? 0,
        p.styles.margins[2] ?? 0,
        p.styles.margins[3] ?? 0,
      ]
      if (!xScrolls) {
        const overRight =
          p.rect.x + p.rect.w - (p.parentRect.x + p.parentRect.w) - Math.max(0, -mRight)
        const overLeft = p.parentRect.x - p.rect.x - Math.max(0, -mLeft)
        if (overRight > tol || overLeft > tol) {
          violations.push({
            check: 'childOverflow',
            testid: nearestTestid(p),
            message: `${p.tag}${p.testid ? `#${p.testid}` : ''} overflows its parent horizontally (${Math.max(overRight, overLeft).toFixed(1)}px)`,
          })
        }
      }
      // Vertical overflow (the docstring promised it; was missing).
      if (!yScrolls && p.parentRect.h > 0) {
        const overBottom =
          p.rect.y + p.rect.h - (p.parentRect.y + p.parentRect.h) - Math.max(0, -mBottom)
        const overTop = p.parentRect.y - p.rect.y - Math.max(0, -mTop)
        if (overBottom > tol || overTop > tol) {
          violations.push({
            check: 'childOverflow',
            testid: nearestTestid(p),
            message: `${p.tag}${p.testid ? `#${p.testid}` : ''} overflows its parent vertically (${Math.max(overBottom, overTop).toFixed(1)}px)`,
          })
        }
      }
    }

    // 2. Spacing on-scale. Uses the tight scale tolerance, NOT the geometric one.
    if (enabled('spacingScale')) {
      // Margins are intentionally NOT grid-checked: getComputedStyle returns the
      // *used* value, so an `auto` margin (e.g. `ms-auto` to push a flex item to
      // the end) resolves to a free-space pixel distance that can never land on a
      // 2px grid — a false positive. Margin collapsing + negative margins compound
      // the unreliability. Paddings + gaps (the real design-token spacing) stay covered.
      const offScale = [
        ...p.styles.paddings,
        ...p.styles.gap,
      ].filter(v => !onScale(v, opts.grid, opts.scaleTolerance))
      if (offScale.length) {
        violations.push({
          check: 'spacingScale',
          testid: nearestTestid(p),
          message: `${p.tag}${p.testid ? `#${p.testid}` : ''} has off-scale spacing: ${offScale.map(v => v.toFixed(1)).join(', ')}px (grid ${opts.grid})`,
        })
      }
      const offRadius = p.styles.radius.filter(
        // A pill/full radius (`rounded-full` → calc(infinity*1px), a huge px
        // value ≥ 9000) is always valid; only flag mid-range off-ramp values,
        // and snap with the TIGHT scale tolerance so off-ramp radii actually fire.
        v => v < 9000 && !opts.radii.some(r => Math.abs(r - v) <= opts.scaleTolerance),
      )
      if (offRadius.length) {
        violations.push({
          check: 'spacingScale',
          testid: nearestTestid(p),
          message: `${p.tag}${p.testid ? `#${p.testid}` : ''} has off-ramp border-radius: ${offRadius.map(v => v.toFixed(1)).join(', ')}px`,
        })
      }
    }

    // 3. Non-block button must not span its container width. Skip when the
    //    button is the SOLE child of its parent — full width is intentional
    //    layout there (e.g. a `block` button in a sized box); the bug class is a
    //    button wrongly stretching among siblings (a toolbar/row).
    if (
      enabled('buttonWidth') &&
      p.isButton &&
      p.parentRect &&
      p.parentChildCount > 1
    ) {
      const isFullWidth = p.rect.w >= p.parentRect.w - tol
      const declaredBlock =
        p.styles.display.includes('block') ||
        p.styles.width === '100%' ||
        p.styles.display === 'flex'
      if (isFullWidth && !declaredBlock && p.parentRect.w > 64) {
        violations.push({
          check: 'buttonWidth',
          testid: nearestTestid(p),
          message: `${p.tag}${p.testid ? `#${p.testid}` : ''} spans its full container width (${p.rect.w.toFixed(0)}px) without being a block element`,
        })
      }
    }

    // 4. Touch target minimum for STANDALONE interactive elements (in-field
    //    affixes like clear-× / password-eye are exempt — WCAG 2.5.8 inline
    //    exception).
    if (
      enabled('touchTarget') &&
      p.isButton &&
      !p.insideControl &&
      // Toggle widgets (switch/checkbox/radio) are conventionally <24px; their
      // effective target includes the associated label, so the visual control
      // size isn't the hit-size invariant. Enforce on plain buttons/links.
      !['switch', 'checkbox', 'radio'].includes(p.role ?? '') &&
      // Tiny icon-only affixes (chip/tag ×, inline remove) are embedded controls
      // with a label elsewhere, not standalone primary targets. Exempt the
      // clearly-sub-20px iconography; still enforce realistic button sizes.
      !(!p.hasTextChild && p.rect.w < 20 && p.rect.h < 20) &&
      // Inline text hyperlinks are exempt (WCAG 2.5.8 "target in a sentence").
      !(p.tag === 'a' && p.hasTextChild && p.styles.display.includes('inline'))
    ) {
      if (
        p.rect.w > 0 &&
        p.rect.h > 0 &&
        (p.rect.w < opts.minTouchTarget - tol ||
          p.rect.h < opts.minTouchTarget - tol)
      ) {
        violations.push({
          check: 'touchTarget',
          testid: nearestTestid(p),
          message: `${p.tag}${p.testid ? `#${p.testid}` : ''} hit size ${p.rect.w.toFixed(0)}×${p.rect.h.toFixed(0)}px is below ${opts.minTouchTarget}px`,
        })
      }
    }

    // 5. Unintended text truncation: content overflows with no ellipsis set.
    if (enabled('textTruncation') && p.hasTextChild) {
      const clipped = p.scrollWidth - p.clientWidth > tol
      // An ellipsis only actually renders when the text is nowrap (single line);
      // text-overflow:ellipsis on wrapping text never shows the ellipsis.
      const hasEllipsis =
        (p.styles.textOverflow === 'ellipsis' &&
          p.styles.whiteSpace === 'nowrap') ||
        // line-clamp-N / Tailwind truncate variants ellipsize via -webkit-line-clamp.
        (p.styles.lineClamp !== 'none' && p.styles.lineClamp !== '')
      // Both `hidden` and `clip` hard-cut content; `clip` (Tailwind overflow-clip)
      // was previously missed.
      const clips =
        p.styles.overflowX === 'hidden' || p.styles.overflowX === 'clip'
      if (clipped && !hasEllipsis && clips) {
        violations.push({
          check: 'textTruncation',
          testid: nearestTestid(p),
          message: `${p.tag}${p.testid ? `#${p.testid}` : ''} clips text (${p.scrollWidth}>${p.clientWidth}) without an ellipsis affordance`,
        })
      }
    }
  }

  // 6. Sibling overlap: direct flow siblings should not intersect.
  if (enabled('siblingOverlap')) {
    violations.push(...detectSiblingOverlap(probes, tol))
  }

  // Collect mode: hand the violations back so the caller can filter against a
  // documented layout baseline before deciding to fail.
  if (options.collect) return violations

  if (violations.length) {
    const grouped = violations
      .map(v => `  • [${v.check}] ${v.message}`)
      .join('\n')
    expect(
      violations.length,
      `Layout violations in ${scopeSelector}:\n${grouped}`,
    ).toBe(0)
  }
  return violations
}

/**
 * Flow siblings (statically-positioned elements sharing a parent) shouldn't
 * overlap. We approximate "siblings" by equal parentRect; absolutely-positioned
 * / overlapping-by-design elements are skipped.
 */
function detectSiblingOverlap(
  probes: ElementProbe[],
  tol: number,
): LayoutViolation[] {
  const out: LayoutViolation[] = []
  const byParent = new Map<number, ElementProbe[]>()
  for (const p of probes) {
    if (p.parentIndex < 0) continue
    if (p.styles.position === 'absolute' || p.styles.position === 'fixed')
      continue
    // Negative margins overlap siblings BY DESIGN (avatar stacks, stacked chips,
    // overlapping cards). Exclude such elements from the overlap check.
    if (p.styles.margins.some(m => m < -tol)) continue
    const arr = byParent.get(p.parentIndex) ?? []
    arr.push(p)
    byParent.set(p.parentIndex, arr)
  }
  for (const sibs of byParent.values()) {
    for (let i = 0; i < sibs.length; i++) {
      for (let j = i + 1; j < sibs.length; j++) {
        const a = sibs[i].rect
        const b = sibs[j].rect
        const ix = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)
        const iy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y)
        if (ix > tol && iy > tol) {
          // Meaningful overlap on BOTH axes (a contained/stacked layout bug).
          const area = ix * iy
          if (area > tol * tol * 16) {
            out.push({
              check: 'siblingOverlap',
              testid: sibs[i].testid ?? sibs[j].testid,
              message: `${sibs[i].tag}${sibs[i].testid ? `#${sibs[i].testid}` : ''} overlaps sibling ${sibs[j].tag}${sibs[j].testid ? `#${sibs[j].testid}` : ''} by ${ix.toFixed(0)}×${iy.toFixed(0)}px`,
            })
          }
        }
      }
    }
  }
  return out
}

/**
 * Assert two locators share a vertical edge (equal `x`) — useful on real pages
 * for "these should left-align" checks. Tolerance is sub-pixel.
 */
export async function assertSameX(
  a: Locator,
  b: Locator,
  tolerance = DEFAULTS.tolerance,
): Promise<void> {
  const [ba, bb] = await Promise.all([a.boundingBox(), b.boundingBox()])
  expect(ba && bb, 'both elements must be visible for alignment check').toBeTruthy()
  expect(
    Math.abs(ba!.x - bb!.x),
    `expected equal left edge, got ${ba!.x} vs ${bb!.x}`,
  ).toBeLessThanOrEqual(tolerance)
}
