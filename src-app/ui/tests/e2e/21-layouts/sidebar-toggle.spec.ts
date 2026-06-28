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

    // Sidebar starts expanded — the toggle reads "Close navigation menu".
    const collapseBtn = page.getByRole('button', {
      name: 'Close navigation menu',
    })
    await expect(collapseBtn).toBeVisible({ timeout: 30000 })
    await expect(collapseBtn).toHaveAttribute('aria-expanded', 'true')
    await expect(collapseBtn).toHaveAttribute('aria-controls', 'app-sidebar')

    // Collapse → the accessible name flips to "Open navigation menu".
    await collapseBtn.click()
    const expandBtn = page.getByRole('button', {
      name: 'Open navigation menu',
    })
    await expect(expandBtn).toBeVisible({ timeout: 10000 })
    await expect(expandBtn).toHaveAttribute('aria-expanded', 'false')

    // Expand again → back to the collapse affordance.
    await expandBtn.click()
    await expect(
      page.getByRole('button', { name: 'Close navigation menu' }),
    ).toBeVisible({ timeout: 10000 })
  })
})
