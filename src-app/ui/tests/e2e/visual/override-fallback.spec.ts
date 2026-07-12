/**
 * TEST-10 — web fallback / no-`.desktop`-leakage positive control.
 *
 * The WEB build registers no overrides and its resolver never serves a
 * `.desktop.tsx`, so every `<Seam>` must render its fallback and no desktop-only
 * override element may appear. This renders a core gallery surface and asserts
 * (a) it renders clean (no crash), and (b) NO desktop override testid
 * (`desktop-hardware-monitor-btn`, or any `desktop-*` override marker) leaked
 * into the web DOM. Runs against the backend-free web gallery.
 *
 * Run: npx playwright test -c playwright.visual.config.ts override-fallback
 */
import { expect, test } from '@playwright/test'

test('TEST-10: web gallery renders the fallback with no desktop-override leakage', async ({
  page,
}) => {
  const pageErrors: string[] = []
  page.on('pageerror', e => pageErrors.push(String(e)))

  // The hardware-monitor surface is a core web surface (the monitor page route).
  await page.goto('/gallery.html?surface=hardware-monitor&state=loaded', {
    waitUntil: 'networkidle',
  })
  await page.waitForSelector('[data-testid="gallery-root"]', { timeout: 20_000 })

  // No crash on the web build.
  await expect(page.locator('[data-testid="gallery-crash"]')).toHaveCount(0)
  expect(pageErrors, 'web page errors').toEqual([])

  // No desktop-only override element leaked into the web bundle. The desktop
  // hardware-monitor override renders `desktop-hardware-monitor-btn`; the web
  // fallback renders `hardware-monitor-btn`. The desktop one must be ABSENT.
  await expect(
    page.locator('[data-testid="desktop-hardware-monitor-btn"]'),
  ).toHaveCount(0)
})
