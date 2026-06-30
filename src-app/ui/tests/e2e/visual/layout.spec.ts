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
import { type Page, expect, test } from '@playwright/test'
import { assertLayoutSane } from '../helpers/layout'
import { isBaselined } from './axe-baseline'
import { isLayoutBaselined } from './layout-baseline'
import { ALL_ACCENTS, VIEWPORTS, openGallery, sectionTestIds } from './_gallery'

// Per-section layout assertion that subtracts documented, pre-existing kit
// findings (layout-baseline.ts) — so the gate catches NEW layout breakage while
// the known backlog (e.g. Tag not wrapping long tokens) doesn't keep it red.
async function assertSectionLayout(page: Page, id: string): Promise<void> {
  const violations = await assertLayoutSane(page.getByTestId(id), {
    checks: { horizontalScroll: false },
    collect: true,
  })
  const fresh = violations.filter(v => !isLayoutBaselined(id, v.check, v.testid))
  expect(
    fresh.length,
    `NEW layout violations in ${id} (beyond baseline):\n${fresh
      .map(v => `  • [${v.check}] ${v.message}`)
      .join('\n')}`,
  ).toBe(0)
}

// Drift guard: the spec inlines the accent list (ALL_ACCENTS in _gallery.ts) to
// stay decoupled from the app module graph. Assert it still equals the app's
// source of truth — ACCENT_ORDER, the exact list the Settings → Appearance
// picker offers — so a newly-added accent can't silently escape the matrix.
// accentPresets.ts is dependency-free, so the relative import loads cleanly in
// the Playwright runner.
test('accent matrix matches Settings → Appearance ACCENT_ORDER (drift guard)', async () => {
  const { ACCENT_ORDER } = await import(
    '../../../src/components/ThemeProvider/accentPresets'
  )
  expect([...ALL_ACCENTS].sort()).toEqual([...ACCENT_ORDER].sort())
})

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
        await assertSectionLayout(page, id)
      })
    }
  })
}

// RTL pass — direction mirroring is a cheap, high-yield bug source (alignment
// flips, icon/affix mis-placement, logical-property gaps that overflow). Run the
// deterministic invariants under dir=rtl at desktop width.
test('layout invariants — RTL (desktop)', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 })
  await openGallery(page, 'light', 'blue', 'rtl')
  const ids = await sectionTestIds(page)
  expect(ids.length).toBeGreaterThan(20)
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
  for (const id of ids) {
    await test.step(id, async () => {
      await assertSectionLayout(page, id)
    })
  }
})

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
        if (isBaselined(v.id, section, selector)) continue
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
