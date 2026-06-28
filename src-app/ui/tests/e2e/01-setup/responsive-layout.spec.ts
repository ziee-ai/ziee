import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — responsive breakpoint transitions of the app shell sidebar
 * (audit id 35e7b34a164645b5). useWindowMinSize drives the layout's
 * desktop↔mobile behavior; crossing the xs breakpoint (<480px) auto-collapses
 * the LeftSidebar (AppLayout.tsx:159-163). The SidebarToggleButton's
 * accessible name reflects the open/collapsed state, giving a stable signal.
 */

test.describe('App shell — responsive sidebar', () => {
  test.describe.configure({ retries: 2 })

  test('crossing the xs breakpoint collapses the sidebar; toggle reopens it', async ({
    page,
    testInfra,
  }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/`)
    await page.waitForLoadState('load')

    // Desktop: the sidebar is open → the toggle offers to CLOSE it.
    await expect(
      page.getByRole('button', { name: 'Close navigation menu' }),
    ).toBeVisible({ timeout: 20000 })

    // Shrink below the xs breakpoint → the effect auto-collapses the sidebar,
    // so the toggle now offers to OPEN it.
    await page.setViewportSize({ width: 375, height: 800 })
    await expect(
      page.getByRole('button', { name: 'Open navigation menu' }),
    ).toBeVisible({ timeout: 10000 })

    // The toggle still works at mobile width: opening flips the control back.
    await page.getByRole('button', { name: 'Open navigation menu' }).click()
    await expect(
      page.getByRole('button', { name: 'Close navigation menu' }),
    ).toBeVisible({ timeout: 10000 })
  })
})
