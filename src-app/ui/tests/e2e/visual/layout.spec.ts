/**
 * Layer A — deterministic layout invariants over the gallery, per viewport, plus
 * an axe-core a11y pass per theme. No screenshot baseline: every assertion here
 * is a bug-by-definition (overflow, off-scale spacing, full-width buttons,
 * overlap, sub-min touch targets, silent truncation) — see
 * `tests/e2e/helpers/layout.ts` and the VISUAL_TESTING_GUIDE.
 *
 * Backend-free: drives the standalone `/dev-gallery.html` entry via the gallery
 * Vite server (playwright.visual.config.ts).
 */
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { assertLayoutSane } from '../helpers/layout'
import { isBaselined } from './axe-baseline'
import { VIEWPORTS, openGallery, sectionTestIds } from './_gallery'

for (const vp of VIEWPORTS) {
  test(`layout invariants — ${vp.name} (${vp.width}px)`, async ({ page }) => {
    await page.setViewportSize({ width: vp.width, height: vp.height })
    await openGallery(page, 'light', 'blue')

    const ids = await sectionTestIds(page)
    expect(ids.length, 'gallery should render sections').toBeGreaterThan(20)

    // Document-level horizontal-scroll check once (cheap), against the root.
    await assertLayoutSane(page.getByTestId('gallery-root'), {
      checks: {
        childOverflow: false,
        siblingOverlap: false,
        spacingScale: false,
        buttonWidth: false,
        touchTarget: false,
        textTruncation: false,
      },
    })

    // Per-section invariants — a violation localizes to one component.
    for (const id of ids) {
      await test.step(id, async () => {
        await assertLayoutSane(page.getByTestId(id), {
          // The root already covers page-level horizontal scroll.
          checks: { horizontalScroll: false },
        })
      })
    }
  })
}

for (const theme of ['light', 'dark'] as const) {
  test(`a11y (axe) — ${theme} theme`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 })
    await openGallery(page, theme, 'blue')

    const results = await new AxeBuilder({ page })
      .include('[data-testid="gallery-root"]')
      .withTags(['wcag2a', 'wcag2aa'])
      .analyze()

    // Resolve each violation node to its gallery section, then drop documented,
    // pre-existing kit findings (axe-baseline.ts). Anything left is NEW → fail.
    const newFindings: string[] = []
    for (const v of results.violations) {
      if (v.impact !== 'serious' && v.impact !== 'critical') continue
      for (const node of v.nodes) {
        const selector = Array.isArray(node.target)
          ? String(node.target[0])
          : String(node.target)
        const section = await page
          .locator(selector)
          .first()
          .evaluate(el =>
            el
              .closest('[data-testid^="gallery-section-"]')
              ?.getAttribute('data-testid') ?? null,
          )
          .catch(() => null)
        if (isBaselined(v.id, section)) continue
        newFindings.push(
          `  • ${v.id} (${v.impact}) in ${section ?? '?'}: ${selector}`,
        )
      }
    }
    expect(
      newFindings.length,
      `NEW axe ${theme} violations (beyond documented baseline):\n${newFindings.join('\n')}`,
    ).toBe(0)
  })
}
