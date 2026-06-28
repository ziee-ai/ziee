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

  test('mobile layout swaps the section rail for a dropdown picker', async ({
    page,
    testInfra,
  }) => {
    // A narrow viewport (< the `sm` breakpoint) flips SettingsPage into its
    // mobile layout: the left section rail is replaced by a single dropdown
    // button in the header (aria-label "Select settings section").
    await page.setViewportSize({ width: 480, height: 900 })
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(page).toHaveURL(/\/settings\/[a-z-]+/, { timeout: 15000 })

    const picker = page.getByRole('button', {
      name: 'Select settings section',
    })
    await expect(picker).toBeVisible({ timeout: 15000 })

    // Opening it reveals the section menu; pick a different section and the
    // route follows.
    await picker.click()
    const menu = page.getByRole('menu')
    await expect(menu).toBeVisible()
    const before = page.url()
    // Click the second selectable menu item (the first is the current section).
    await menu.getByRole('menuitem').nth(1).click()
    await expect
      .poll(() => page.url(), { timeout: 15000 })
      .not.toBe(before)
    await expect(page).toHaveURL(/\/settings\/[a-z-]+/)
  })
})
