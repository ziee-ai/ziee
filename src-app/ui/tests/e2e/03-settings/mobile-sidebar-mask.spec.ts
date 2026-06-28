import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — clicking the mobile overlay mask closes the sidebar
 * (AppLayout.handleMaskClick → setSidebarCollapsed(true)).
 *
 * Audit gap: the mobile mask-click-to-close behavior was untested. On an
 * xs viewport the sidebar is a modal overlay; clicking its mask must collapse
 * it (the sidebar dialog's aria-hidden flips back to true).
 */

test.use({ viewport: { width: 390, height: 844 } })

test.describe('Layout — mobile sidebar mask', () => {
  test('clicking the mask closes the open mobile sidebar', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/profile`)

    const sidebar = page.locator('#app-sidebar')
    // Mobile: the sidebar starts collapsed (aria-hidden=true).
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', {
      timeout: 30000,
    })

    // Open the overlay.
    await page.getByRole('button', { name: 'Open navigation menu' }).click()
    await expect(sidebar).toHaveAttribute('aria-hidden', 'false', {
      timeout: 10000,
    })

    // Click the mask → the overlay closes.
    await page.locator('[data-sidebar-mask]').click()
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', {
      timeout: 10000,
    })
  })
})
