import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

// audit id all-6b38bee63550 — pressing Escape closes the mobile sidebar.
// The mobile sidebar is now a Sheet (Base-UI Dialog) whose content is only
// mounted while open, so open/closed is asserted via visibility (not an
// aria-hidden attribute on a persistent node). Escape-to-close is provided by
// the Dialog primitive.
test.describe('Mobile sidebar — Escape to close', () => {
  test('Escape collapses the open mobile sidebar', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.setViewportSize({ width: 400, height: 800 })
    await page.goto(`${baseURL}/`)

    const sidebar = byTestId(page, 'app-sidebar')
    // Mobile boot → the Sheet is closed, so its content is not shown.
    await expect(sidebar).toBeHidden({ timeout: 10000 })

    // Open it via the toggle button.
    await byTestId(page, 'layout-sidebar-toggle-button').click()
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Escape must collapse it again.
    await page.keyboard.press('Escape')
    await expect(sidebar).toBeHidden({ timeout: 10000 })
  })
})
