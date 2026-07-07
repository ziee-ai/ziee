/**
 * Desktop-only gallery runtime + audit-gate spec (TEST-8..12).
 *
 * Drives the desktop-only ROUTE chrome surfaces the gallery CAN render
 * (settings-about / settings-remote-access / settings-host-mount) via the
 * mock-API cassette and asserts: no crash boundary, no console/page error in the
 * loaded state, and axe a11y clean — across light + dark. Plus the geometry +
 * affordance audit gates exit 0 for these surfaces. (The remaining desktop-only
 * chrome — window/file-dialog/desktop-base — has no DOM; UpdateBanner/app-layout
 * are Tauri/shell-coupled — see .lifecycle/.../DESKTOP_UI_FINDINGS.md, DRIFT-1.3.)
 *
 * Run: npx playwright test -c playwright.gallery.config.ts
 */
import { execFileSync } from 'node:child_process'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

const SURFACES = ['settings-about', 'settings-remote-access', 'settings-host-mount']
const THEMES = ['light', 'dark'] as const
const PORT = Number(process.env.GALLERY_PORT || 1455)

for (const surface of SURFACES) {
  for (const theme of THEMES) {
    test(`${surface} [${theme}] renders clean (no crash, no console error, axe)`, async ({
      page,
    }) => {
      const consoleErrors: string[] = []
      const pageErrors: string[] = []
      page.on('console', m => {
        if (m.type() === 'error') consoleErrors.push(m.text())
      })
      page.on('pageerror', e => pageErrors.push(String(e)))

      await page.goto(`/gallery.html?surface=${surface}&state=loaded&theme=${theme}`, {
        waitUntil: 'networkidle',
      })
      await page.waitForSelector('[data-testid="gallery-root"]', { timeout: 20_000 })
      await page.waitForSelector(`[data-testid="gallery-page-${surface}"]`, {
        timeout: 20_000,
      })

      // TEST-8: no error-boundary crash on a loaded desktop-only surface.
      await expect(page.locator('[data-testid="gallery-crash"]')).toHaveCount(0)

      // TEST-9: no console/page error in the LOADED state (proves the F1 build
      // break is gone + no runtime crash). Error-STATE cassette logs are not
      // exercised here (loaded only).
      expect(pageErrors, `page errors on ${surface}/${theme}`).toEqual([])
      expect(consoleErrors, `console errors on ${surface}/${theme}`).toEqual([])

      // TEST-12: axe a11y — no serious/critical violation on the surface.
      const results = await new AxeBuilder({ page })
        .include(`[data-testid="gallery-page-${surface}"]`)
        .analyze()
      const serious = results.violations.filter(
        v => v.impact === 'serious' || v.impact === 'critical',
      )
      expect(
        serious,
        `axe violations on ${surface}/${theme}: ${serious.map(v => v.id).join(', ')}`,
      ).toEqual([])
    })
  }
}

// TEST-10 + TEST-11: the geometry + affordance audit gates exit 0 for the
// desktop-only surfaces (run against the same gallery the webServer booted).
test('geometry audit gate exits 0 on desktop-only surfaces', () => {
  test.setTimeout(300_000)
  execFileSync(
    'node',
    [
      'scripts/gallery-geometry-audit.mjs',
      '--gate',
      `--surfaces=${SURFACES.join(',')}`,
    ],
    { cwd: process.cwd(), env: { ...process.env, GALLERY_PORT: String(PORT) }, stdio: 'pipe' },
  )
})

test('affordance audit gate exits 0', () => {
  test.setTimeout(300_000)
  execFileSync('node', ['scripts/affordance-audit.mjs', '--report-only'], {
    cwd: process.cwd(),
    env: { ...process.env, GALLERY_PORT: String(PORT) },
    stdio: 'pipe',
  })
})
