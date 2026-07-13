/**
 * TEST-17 (ITEM-15) — desktop gallery per-module seed parity.
 *
 * Proves (against the backend-free desktop `/gallery.html`):
 *   1. desktop-ONLY modules render POPULATED from their own `gallery.tsx`
 *      (host-mount's policy cassette),
 *   2. SHARED web modules render POPULATED in the desktop gallery via the
 *      cross-workspace per-module cassette (`module-seed.ts` pulls the web
 *      workspace's `MODULE_CASSETTE`) — i.e. the desktop gallery inherits the
 *      web seed for free.
 */
import { expect, test, type Page } from '@playwright/test'

async function renders(page: Page, slug: string) {
  const errors: string[] = []
  page.on('pageerror', e => errors.push(String(e)))
  page.on('console', m => {
    if (m.type() === 'error' && !/Failed to load resource/i.test(m.text()))
      errors.push(m.text())
  })
  await page.goto(`/gallery.html?surface=${slug}&theme=light&accent=blue`)
  await page.getByTestId('gallery-root').waitFor()
  const frame = page.getByTestId(`gallery-page-${slug}`)
  await frame.waitFor({ timeout: 15000 })
  await page.waitForTimeout(1200)
  await expect(frame.getByTestId('gallery-crash')).toHaveCount(0)
  const text = (await frame.innerText()).trim()
  expect(text.length, `"${slug}" rendered only chrome`).toBeGreaterThan(40)
  expect(errors, `console/page errors on ${slug}: ${errors.join(' | ')}`).toEqual([])
}

// 1. Desktop-only module, seeded by its own gallery.tsx.
test('TEST-17: desktop-only settings-host-mount renders populated (own seed)', async ({
  page,
}) => {
  await renders(page, 'settings-host-mount')
})

// 2. Shared web module, seeded via the cross-workspace MODULE_CASSETTE.
test('TEST-17: shared settings-users renders populated in the desktop gallery', async ({
  page,
}) => {
  await renders(page, 'settings-users')
})
