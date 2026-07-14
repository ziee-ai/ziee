/**
 * TEST-12 (ITEM-13) — the previously-DARK gap surfaces now render populated:
 * app SetupPage (App.getSetupStatus), /settings/sessions (Auth.getSessionSettings),
 * code-sandbox rootfs section (CodeSandbox.listRootfsVersions), and /files/:fileId
 * (File.get). Backend-free, against `/gallery.html`.
 */
import { test, expect, type Page } from '@playwright/test'

async function assertRenders(page: Page, slug: string, extra = '') {
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
  const frame = page.getByTestId(`gallery-page-${slug}`)
  await frame.waitFor({ timeout: 15000 })
  await page.waitForTimeout(1200)
  await expect(frame.getByTestId('gallery-crash')).toHaveCount(0)
  // Measure the RENDERED-COMPONENT subtree, not the section (which always has
  // ~50 chars of gallery chrome — measuring it passes on an empty seed).
  const text = (await frame.locator('[data-gallery-frame]').innerText()).trim()
  expect(text.length, `"${slug}" rendered empty (only chrome)`).toBeGreaterThan(20)
  expect(errors, `console/page errors on ${slug}: ${errors.join(' | ')}`).toEqual([])
}

// The app setup page needs a logged-out seed (it redirects when authenticated).
test('TEST-12: app SetupPage renders (getSetupStatus seeded)', async ({ page }) => {
  await assertRenders(page, 'setup', '&auth=none')
})

test('TEST-12: /settings/sessions renders (session settings seeded)', async ({
  page,
}) => {
  await assertRenders(page, 'settings-sessions')
})

test('TEST-12: /settings/sandbox rootfs section renders (listRootfsVersions seeded)', async ({
  page,
}) => {
  await assertRenders(page, 'settings-sandbox')
})

// A required-param detail route is skipped unless the URL pins the param (the
// gallery's isolated-detail convention, like conversationId/projectId).
test('TEST-12: /files/:fileId viewer renders (File.get seeded)', async ({ page }) => {
  await assertRenders(page, 'files-detail', '&fileId=gallery-file-1')
})
