import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the collapsed-sidebar state survives client-side navigation.
 *
 * Audit gap: AppLayout.store holds `isSidebarCollapsed` at module scope
 * (deliberately NOT a per-mount useRef, so a route change doesn't reset it
 * — see the store comment), but no test navigated to prove it. This
 * collapses the sidebar, then navigates to another route via in-app routing
 * (not a reload) and asserts the sidebar is still collapsed.
 */

test.describe('Layout — sidebar collapse survives navigation', () => {
  test('collapsing then navigating keeps the sidebar collapsed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Land on an authed route with the app shell (NOT profile — we navigate
    // to profile below to cross a route boundary).
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // Collapse the sidebar.
    await page
      .getByRole('button', { name: 'Close navigation menu' })
      .click()
    const collapsed = page.getByRole('button', { name: 'Open navigation menu' })
    await expect(collapsed).toBeVisible()

    // Client-side navigate via the profile widget dropdown (NOT a reload).
    await page.getByTestId('user-profile-widget').click()
    await page.getByRole('menuitem', { name: 'Profile' }).click()
    await expect(page).toHaveURL(/\/settings\/profile$/)

    // The collapsed state persisted across the route change (still showing
    // the "Open navigation menu" affordance, never reset to expanded).
    await expect(
      page.getByRole('button', { name: 'Open navigation menu' }),
    ).toBeVisible()
  })
})
