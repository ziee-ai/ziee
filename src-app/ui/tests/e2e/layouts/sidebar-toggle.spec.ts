import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — app-layout sidebar collapse/expand (the `layouts` module had NO spec).
 *
 * `SidebarToggleButton` renders a Button whose accessible name flips between
 * "Close navigation menu" (expanded) and "Open navigation menu" (collapsed) and
 * carries `aria-expanded` + `aria-controls="app-sidebar"`. Clicking it calls
 * `Stores.AppLayout.toggleSidebar()`. This drives that through the real UI.
 */

test.describe('App layout — sidebar toggle', () => {
  test('clicking the toggle collapses then expands the sidebar', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // Sidebar starts expanded — the toggle exposes aria-expanded="true".
    const toggleBtn = page.getByTestId('layout-sidebar-toggle-button')
    await expect(toggleBtn).toBeVisible({ timeout: 30000 })
    await expect(toggleBtn).toHaveAttribute('aria-expanded', 'true')
    await expect(toggleBtn).toHaveAttribute('aria-controls', 'app-sidebar')

    // Collapse → aria-expanded flips to "false".
    await toggleBtn.click()
    await expect(toggleBtn).toHaveAttribute('aria-expanded', 'false', {
      timeout: 10000,
    })

    // Expand again → back to the expanded state.
    await toggleBtn.click()
    await expect(toggleBtn).toHaveAttribute('aria-expanded', 'true', {
      timeout: 10000,
    })
  })
})
