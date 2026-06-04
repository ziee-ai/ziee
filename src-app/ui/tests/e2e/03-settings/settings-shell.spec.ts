import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * Settings shell routing.
 *
 * Regression for the settings layout flash: opening /settings used to paint the
 * app's AppLayout (chat sider) and then swap to the settings layout, because
 * the bare /settings route declared `element: SettingsLayout, layout:
 * AppLayoutDef` — AppLayout rendered twice (route layout + inside the lazy
 * SettingsLayout) and in a different layout group than its /settings/* sub-
 * pages. /settings now uses SettingsLayoutDef like every sub-page, so there's a
 * single AppLayout and no layout-group switch on the redirect.
 *
 * The sub-second flash itself is hard to assert; this guards the observable
 * contract — /settings redirects to the first section, renders, and mounts the
 * app shell exactly once.
 */
test.describe('Settings shell', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('opening /settings redirects to a section and renders one app shell', async ({
    page,
    testInfra,
  }) => {
    await page.goto(`${testInfra.baseURL}/settings`)

    // Bare /settings redirects to the first permitted section (not stuck at /settings).
    await expect(page).toHaveURL(/\/settings\/[a-z-]+/, { timeout: 15000 })

    // The settings content rendered (a section heading is shown).
    await expect(page.getByRole('heading').first()).toBeVisible({ timeout: 15000 })

    // Exactly one AppLayout sider (#app-sidebar is unique to AppLayout) — i.e.
    // the settings layout is not nested inside a second AppLayout.
    await expect(page.locator('#app-sidebar')).toHaveCount(1)
  })
})
