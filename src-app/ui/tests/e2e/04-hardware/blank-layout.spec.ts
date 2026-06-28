import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — `BlankLayout` (audit 38f65974c5c3).
 *
 * `layouts/blank/BlankLayout.tsx` is the chrome-less layout: it renders its
 * children inside a single top-level `<main style="display:contents">`
 * landmark and applies NO app shell (no `#app-sidebar`, no sidebar toggle).
 * It backs the `/hardware-monitor` popup route (hardware/module.tsx → the
 * route with `layout: BlankLayout`). No prior test asserted that this route
 * actually renders without the app chrome — only that the popup opens.
 *
 * This navigates directly to `/hardware-monitor` and proves the blank layout
 * is in effect (app sidebar/toggle absent, single `main` landmark, monitor
 * content present), contrasted against a normal app route that DOES carry the
 * full app shell.
 */

test.describe('Layouts — BlankLayout (/hardware-monitor)', () => {
  test('renders the route chrome-less: no app sidebar, just a main landmark + content', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // --- Positive control: a normal app route carries the full app shell. ---
    await page.goto(`${baseURL}/settings/hardware`)
    await expect(page.locator('#app-sidebar')).toHaveCount(1, { timeout: 30000 })

    // --- BlankLayout route: the app chrome is gone. ---
    await page.goto(`${baseURL}/hardware-monitor`)

    // The dedicated monitor view renders (sr-only h1 from HardwareMonitor.tsx).
    await expect(
      page.getByRole('heading', { name: 'Hardware Monitor' }),
    ).toBeAttached({ timeout: 30000 })

    // No app shell: the sidebar element and its toggle button are both absent.
    await expect(page.locator('#app-sidebar')).toHaveCount(0)
    await expect(
      page.locator('button[aria-controls="app-sidebar"]'),
    ).toHaveCount(0)

    // BlankLayout supplies exactly one top-level `main` landmark for the
    // chrome-less page (`<main style="display:contents">`).
    await expect(page.getByRole('main')).toHaveCount(1)
  })
})
