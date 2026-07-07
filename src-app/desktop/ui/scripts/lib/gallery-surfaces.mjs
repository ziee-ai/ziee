/**
 * SINGLE SOURCE of gallery-surface enumeration for the Node capture + coverage
 * tooling — the counterpart to `src/dev/gallery/surfaces.ts`.
 *
 * Every capture/coverage script drives Playwright and must enumerate ALL FOUR
 * surface classes (pages / overlays / deep / seeded). Before this helper, the
 * capture scripts enumerated ONLY the browse-canvas page DOM and therefore
 * silently skipped overlays/deep/seeded. Now they all call `enumerateSurfaces()`,
 * which reads `window.__GALLERY_LIST_ALL_SURFACES__()` (falling back to the DOM +
 * per-class globals for an older gallery bundle), so no class is ever missed.
 */

/**
 * Enumerate every gallery surface class. `page` must be a Playwright Page already
 * navigated to the browse canvas (no `?surface=`), or pass `base` to navigate.
 * Returns `{ pages, overlays, deep, seeded }` (string[] each).
 */
export async function enumerateSurfaces(page, base) {
  if (base) {
    await page.goto(base, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(5000)
  }
  return page.evaluate(() => {
    const fn = window.__GALLERY_LIST_ALL_SURFACES__
    if (typeof fn === 'function') {
      const r = fn()
      return {
        pages: r.pages || [],
        overlays: r.overlays || [],
        deep: r.deep || [],
        seeded: r.seeded || [],
      }
    }
    // Fallback for an older gallery bundle without the unified function.
    const overlays = window.__GALLERY_OVERLAYS__ || []
    const deep = window.__GALLERY_DEEP_STATES__ || []
    const seeded = window.__GALLERY_SEEDED__ || []
    const special = new Set([...overlays, ...deep, ...seeded])
    const pages = Array.from(
      document.querySelectorAll('[data-testid^="gallery-page-"]'),
    )
      .map(el => (el.getAttribute('data-testid') || '').replace('gallery-page-', ''))
      .filter(id => id && !special.has(id))
    return { pages: [...new Set(pages)], overlays, deep, seeded }
  })
}

/**
 * Flatten the surface classes into a list of CAPTURE CELLS — one screenshot each.
 * Pages get the full data-state set (`states`); the interaction-only classes each
 * render once via `?surface=<slug>` (their single natural state).
 *
 * Each cell: `{ slug, cls, state }` where `cls ∈ page|overlay|deep|seeded`.
 */
export function captureCells(classes, { states = ['loaded', 'empty', 'error'] } = {}) {
  const cells = []
  for (const slug of classes.pages)
    for (const state of states) cells.push({ slug, cls: 'page', state })
  for (const slug of classes.overlays) cells.push({ slug, cls: 'overlay', state: 'open' })
  for (const slug of classes.deep) cells.push({ slug, cls: 'deep', state: 'deep' })
  for (const slug of classes.seeded) cells.push({ slug, cls: 'seeded', state: 'seeded' })
  return cells
}

/** Build the single-surface render URL for a capture cell. */
export function cellUrl(base, cell, { theme } = {}) {
  const p = new URLSearchParams()
  p.set('surface', cell.slug)
  // Only data-state pages honor `&state=`; overlays open on mount, deep/seeded
  // seed their own transient state.
  if (cell.cls === 'page') p.set('state', cell.state)
  else if (cell.cls === 'overlay') p.set('state', 'open')
  if (theme) p.set('theme', theme)
  return `${base}?${p.toString()}`
}

/** Total surface count across all classes (for capture-completeness reporting). */
export function surfaceCount(classes) {
  return (
    classes.pages.length +
    classes.overlays.length +
    classes.deep.length +
    classes.seeded.length
  )
}
