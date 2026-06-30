/**
 * Layer B — visual-regression snapshots over the gallery matrix.
 *
 * For each (viewport × theme × accent) cell we snapshot EACH `gallery-section-*`
 * individually (not one giant page shot) so a diff localizes to a single
 * component and stays small. Baselines are blessed once with
 * `--update-snapshots`; thereafter any unintended visual change is a pixel diff
 * to review. `maxDiffPixelRatio` (config) absorbs font-AA noise.
 *
 * Backend-free via the gallery Vite server (playwright.visual.config.ts).
 * See VISUAL_TESTING_GUIDE §3.
 */
import { expect, test } from '@playwright/test'
import {
  MATRIX_ACCENTS,
  THEMES,
  VIEWPORTS,
  openGallery,
  sectionTestIds,
} from './_gallery'

// Snapshot matrix. Accent only meaningfully repaints accent-bearing components,
// but sweeping a few proves components hold under any primary hue. Keep the cell
// count bounded: viewports × themes × accents = 3 × 2 × 3 = 18 combos.
for (const vp of VIEWPORTS) {
  for (const theme of THEMES) {
    for (const accent of MATRIX_ACCENTS) {
      test(`snapshot — ${vp.name}/${theme}/${accent}`, async ({ page }) => {
        await page.setViewportSize({ width: vp.width, height: vp.height })
        await openGallery(page, theme, accent)

        const ids = await sectionTestIds(page)
        expect(ids.length).toBeGreaterThan(20)

        for (const id of ids) {
          const section = page.getByTestId(id)
          await section.scrollIntoViewIfNeeded()
          await expect(section).toHaveScreenshot(
            `${id}-${vp.name}-${theme}-${accent}.png`,
          )
        }
      })
    }
  }
}
