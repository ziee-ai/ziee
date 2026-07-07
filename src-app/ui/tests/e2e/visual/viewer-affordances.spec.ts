/**
 * TEST-11 (ITEM-11) — backend-free coverage of the new viewer shell chrome via
 * the gallery's file-preview overlay. Runs under playwright.visual.config.ts
 * (gallery Vite server, no backend), mirroring overlays.spec.ts.
 *
 * The functional zoom/find/wrap flows are covered against the real backend
 * (tests/e2e/file/*). Here we assert the shell affordances render open + sane in
 * the gallery so the runtime-health / contrast gate has a stable surface.
 */
import { expect, test } from '@playwright/test'
import { assertLayoutSane } from '../helpers/layout'

const FILE_PREVIEW = '/gallery.html?surface=overlay-file-preview-drawer&state=open&theme=light'

for (const theme of ['light', 'dark'] as const) {
  test(`file preview shell chrome renders — ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(
      FILE_PREVIEW.replace('theme=light', `theme=${theme}`),
    )
    const dialog = page.getByRole('dialog').first()
    await dialog.waitFor({ state: 'visible' })

    // Shell affordances present for any file (footer action row).
    await expect(page.getByTestId('file-viewer-open-tab-btn')).toBeVisible()
    await expect(page.getByTestId('file-viewer-fullpage-btn')).toBeVisible()

    // Dense header/footer surface — assert no horizontal overflow / layout bugs.
    await assertLayoutSane(dialog, { checks: { horizontalScroll: false } })
  })
}
