import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — settings shell SIDEBAR navigation. settings-shell.spec only asserts the
 * bare-/settings redirect + a single app shell; it never clicks the sidebar
 * menu. This drives the menu: clicking section entries navigates to the matching
 * /settings/<section> route and renders that section.
 */

test.describe('Settings — sidebar navigation', () => {
  test('clicking sidebar entries navigates between settings sections', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(page).toHaveURL(/\/settings\/[a-z-]+/, { timeout: 15000 })

    // Navigate to General via the sidebar menu.
    await page.getByRole('menuitem', { name: 'General' }).click()
    await expect(page).toHaveURL(/\/settings\/general/, { timeout: 15000 })
    await expect(
      page.locator('#theme-form [aria-label="Theme"]').first(),
    ).toBeVisible({ timeout: 15000 })

    // Navigate to Hardware (an admin section) via the sidebar menu.
    await page.getByRole('menuitem', { name: 'Hardware' }).click()
    await expect(page).toHaveURL(/\/settings\/hardware/, { timeout: 15000 })
    await expect(page.getByText('Real-time Monitoring:')).toBeVisible({
      timeout: 15000,
    })

    // And back to General — the menu keeps working after a section switch.
    await page.getByRole('menuitem', { name: 'General' }).click()
    await expect(page).toHaveURL(/\/settings\/general/, { timeout: 15000 })
  })
})
