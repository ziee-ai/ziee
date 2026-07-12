/**
 * Desktop UI override e2e (TEST-9 + TEST-11) — runs against the mock-cassette
 * gallery (backend-free), so it boots the REAL desktop bundle where the
 * `.desktop.tsx` resolver has already swapped in the desktop overrides at build
 * time. These surfaces render THROUGH the relocated desktop overrides
 * (`SettingsPage.desktop.tsx` shell + `AboutPage`/`RemoteAccessPage`/host-mount
 * pages), so a clean render proves the co-located `.desktop.tsx` migration works
 * end-to-end in a real browser — not just in tsc/unit tests.
 *
 * Run: npx playwright test -c playwright.gallery.config.ts gallery-desktop-override
 */
import { expect, test, type Page } from '@playwright/test'

// Desktop-only route surfaces that render inside the relocated desktop
// SettingsPage.desktop.tsx shell.
const SURFACES = ['settings-about', 'settings-remote-access', 'settings-host-mount']
const THEMES = ['light', 'dark'] as const

// The vite DEV server pre-bundles deps on first load and returns 504 "Outdated
// Optimize Dep" (with a full-reload signal) until it settles; navigate + reload
// until the gallery root renders. Resilient to a cold optimize cache.
async function gotoSurface(page: Page, url: string) {
  for (let attempt = 0; attempt < 5; attempt++) {
    await page.goto(url, { waitUntil: 'domcontentloaded' })
    try {
      await page.waitForSelector('[data-testid="gallery-root"]', { timeout: 15_000 })
      return
    } catch {
      // 504/optimize-in-progress → reload picks up the freshly-optimized deps.
      await page.waitForTimeout(1500)
    }
  }
  await page.waitForSelector('[data-testid="gallery-root"]', { timeout: 15_000 })
}

for (const surface of SURFACES) {
  for (const theme of THEMES) {
    // TEST-9: the migrated .desktop.tsx overrides render (no crash) in the real
    // desktop bundle. TEST-11: zero console/page errors on the override surface.
    test(`TEST-9/11: ${surface} [${theme}] renders through desktop overrides, clean`, async ({
      page,
    }) => {
      const consoleErrors: string[] = []
      const pageErrors: string[] = []
      // Ignore the vite DEV-server dep-optimization noise (504/reload during the
      // cold first load) — it is not an application error.
      const isDevNoise = (t: string) =>
        /Outdated Optimize Dep|504|Failed to load resource|net::ERR_ABORTED/i.test(t)
      page.on('console', m => {
        if (m.type() === 'error' && !isDevNoise(m.text())) consoleErrors.push(m.text())
      })
      page.on('pageerror', e => {
        if (!isDevNoise(String(e))) pageErrors.push(String(e))
      })

      await gotoSurface(
        page,
        `/gallery.html?surface=${surface}&state=loaded&theme=${theme}`,
      )
      await page.waitForSelector(`[data-testid="gallery-page-${surface}"]`, {
        timeout: 20_000,
      })

      // TEST-9: no ErrorBoundary crash rendering through the desktop overrides.
      await expect(page.locator('[data-testid="gallery-crash"]')).toHaveCount(0)
      // TEST-11: zero runtime errors on the override surface.
      expect(pageErrors, `page errors on ${surface}/${theme}`).toEqual([])
      expect(consoleErrors, `console errors on ${surface}/${theme}`).toEqual([])
    })
  }
}
