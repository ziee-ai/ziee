import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

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

    // The settings content rendered (the section title is shown).
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 15000,
    })

    // Exactly one AppLayout sider (app-sidebar is unique to AppLayout) — i.e.
    // the settings layout is not nested inside a second AppLayout.
    await expect(byTestId(page, 'app-sidebar')).toHaveCount(1)
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

    const picker = byTestId(page, 'settings-mobile-dropdown-trigger')
    await expect(picker).toBeVisible({ timeout: 15000 })

    // Opening it reveals the section menu; pick a section that differs from the
    // current one and the route follows.
    await picker.click()
    await expect(byTestId(page, 'settings-mobile-dropdown')).toBeVisible()
    const before = page.url()
    // General and Profile are always present; pick whichever isn't current.
    const targetPath = before.includes('/general') ? 'profile' : 'general'
    await byTestId(page, `settings-mobile-dropdown-item-${targetPath}`).click()
    await expect
      .poll(() => page.url(), { timeout: 15000 })
      .not.toBe(before)
    await expect(page).toHaveURL(/\/settings\/[a-z-]+/)
  })

  test('desktop sidebar menu navigates between sections', async ({
    page,
    testInfra,
  }) => {
    // Desktop viewport → the section rail (antd Menu) is shown, not the
    // mobile dropdown.
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(`${testInfra.baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 15000,
    })

    // Click the "MCP Servers" section in the sidebar → route + content follow.
    await byTestId(page, 'settings-nav-menu-item-mcp-servers').click()
    await expect(page).toHaveURL(/\/settings\/mcp-servers$/)

    // And back to Profile via the sidebar.
    await byTestId(page, 'settings-nav-menu-item-profile').click()
    await expect(page).toHaveURL(/\/settings\/profile$/)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible()
  })
})
