import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
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

    // Desktop: the sidebar is open → the toggle reports expanded.
    const toggle = byTestId(page, 'layout-sidebar-toggle-button')
    await expect(toggle).toBeVisible({ timeout: 20000 })
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')

    // Shrink below the xs breakpoint → the effect auto-collapses the sidebar,
    // so the toggle now reports collapsed.
    await page.setViewportSize({ width: 375, height: 800 })
    await expect(toggle).toHaveAttribute('aria-expanded', 'false', {
      timeout: 10000,
    })

    // The toggle still works at mobile width: clicking reopens the sidebar.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'true', {
      timeout: 10000,
    })
  })
})
