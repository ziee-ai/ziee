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
import { expect, test } from '@playwright/test'

// Desktop-only route surfaces that render inside the relocated desktop
// SettingsPage.desktop.tsx shell.
const SURFACES = ['settings-about', 'settings-remote-access', 'settings-host-mount']
const THEMES = ['light', 'dark'] as const

for (const surface of SURFACES) {
  for (const theme of THEMES) {
    // TEST-9: the migrated .desktop.tsx overrides render (no crash) in the real
    // desktop bundle. TEST-11: zero console/page errors on the override surface.
    test(`TEST-9/11: ${surface} [${theme}] renders through desktop overrides, clean`, async ({
      page,
    }) => {
      const consoleErrors: string[] = []
      const pageErrors: string[] = []
      page.on('console', m => {
        if (m.type() === 'error') consoleErrors.push(m.text())
      })
      page.on('pageerror', e => pageErrors.push(String(e)))

      await page.goto(
        `/gallery.html?surface=${surface}&state=loaded&theme=${theme}`,
        { waitUntil: 'networkidle' },
      )
      await page.waitForSelector('[data-testid="gallery-root"]', { timeout: 20_000 })
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
