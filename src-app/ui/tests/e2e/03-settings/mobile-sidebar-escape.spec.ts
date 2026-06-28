import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-6b38bee63550 — pressing Escape closes the mobile sidebar
// (AppLayout.tsx:268-272: onKeyDown Escape → setSidebarCollapsed(true)); no E2E
// sent Escape in a mobile viewport. We open the auto-collapsed mobile sidebar
// via its toggle, then assert Escape re-collapses it.
test.describe('Mobile sidebar — Escape to close', () => {
  test('Escape collapses the open mobile sidebar', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.setViewportSize({ width: 400, height: 800 })
    await page.goto(`${baseURL}/`)

    const sidebar = page.locator('#app-sidebar')
    // Mobile boot → sidebar is auto-collapsed.
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })

    // Open it via the toggle button (semantic aria-label).
    await page.getByRole('button', { name: 'Open navigation menu' }).click()
    await expect(sidebar).not.toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })

    // Escape must collapse it again.
    await page.keyboard.press('Escape')
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
  })
})
