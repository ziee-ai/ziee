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

const FILE_PREVIEW = '/gallery.html?surface=overlay-file-preview-drawer&state=open&theme=light'

for (const theme of ['light', 'dark'] as const) {
  test(`file preview shell chrome renders — ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(FILE_PREVIEW.replace('theme=light', `theme=${theme}`))

    // The new shell affordances (open-in-new-tab + full-page) render for any file
    // in the preview drawer's action row — backend-free, in both themes. The
    // surface's layout/contrast is separately covered by the runtime-health gate.
    await expect(page.getByTestId('file-viewer-open-tab-btn')).toBeVisible()
    await expect(page.getByTestId('file-viewer-fullpage-btn')).toBeVisible()
    // Both shell buttons carry an accessible name (no unnamed icon buttons).
    await expect(page.getByRole('button', { name: 'Open file in new tab' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Open file full page' })).toBeVisible()
  })
}
