/**
 * TEST-10 (ITEM-12) — the 5 previously-UNSEEDED modules now render POPULATED in
 * the gallery (not empty/crash): js-tool, knowledge-base, notification,
 * scheduler, voice.
 * TEST-11 (ITEM-12) — the 3 previously-UNWIRED overlays render OPEN.
 *
 * Backend-free, against `/gallery.html` (playwright.visual.config boots vite).
 */
import { test, expect, type Page } from '@playwright/test'

async function gotoSurface(page: Page, slug: string, extra = '') {
  const errors: string[] = []
  page.on('pageerror', e => errors.push(String(e)))
  page.on('console', m => {
    // Ignore benign network asset failures (non-/api resources hit vite, not the
    // mock) — the runtime-health gate filters these too; a real bug is a JS error.
    if (m.type() === 'error' && !/Failed to load resource/i.test(m.text()))
      errors.push(m.text())
  })
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue${extra}`)
  await page.getByTestId('gallery-root').waitFor()
  await page.getByTestId(`gallery-page-${slug}`).waitFor({ timeout: 15000 })
  // Let the seeded stores settle.
  await page.waitForTimeout(1500)
  return errors
}

// TEST-10 — one case per newly-seeded module page.
const NEW_PAGES = [
  'settings-js-tool',
  'knowledge',
  'notifications',
  'scheduled-tasks',
  'settings-voice',
]

for (const slug of NEW_PAGES) {
  test(`TEST-10: newly-seeded page "${slug}" renders populated, no crash`, async ({
    page,
  }) => {
    const errors = await gotoSurface(page, slug)
    const frame = page.getByTestId(`gallery-page-${slug}`)

    // No error-boundary crash marker anywhere on the surface.
    await expect(frame.getByTestId('gallery-crash')).toHaveCount(0)

    // The frame has real rendered content (more than just the gallery chrome
    // label) — a heading, a control, or a data row.
    const text = (await frame.innerText()).trim()
    expect(text.length, `"${slug}" rendered only chrome`).toBeGreaterThan(40)

    // No runtime console/page errors on this surface.
    expect(errors, `console/page errors on ${slug}: ${errors.join(' | ')}`).toEqual(
      [],
    )
  })
}

// TEST-11 — the 3 newly-wired overlays render OPEN (portaled to body).
const NEW_OVERLAYS = [
  'overlay-knowledge-base-form-drawer',
  'overlay-scheduled-task-form-drawer',
  'overlay-upload-model-drawer',
]

for (const slug of NEW_OVERLAYS) {
  test(`TEST-11: newly-wired overlay "${slug}" renders open`, async ({ page }) => {
    const errors = await gotoSurface(page, slug, '&state=open')
    // An open Base-UI Sheet/Dialog portals a role=dialog to the body.
    await expect(page.getByRole('dialog').first()).toBeVisible({ timeout: 15000 })
    await expect(page.getByTestId('gallery-crash')).toHaveCount(0)
    expect(errors, `console/page errors on ${slug}: ${errors.join(' | ')}`).toEqual(
      [],
    )
  })
}
