import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-eb2b4abb33fd — responsive/mobile breakpoint transitions were
// untested (0 of the E2E specs cross a breakpoint). The app sidebar
// (#app-sidebar) auto-collapses when the viewport crosses into the `xs` mobile
// breakpoint (AppLayout.tsx:159-163 useEffect on windowMinSize.xs); on mobile
// the collapsed sidebar is marked aria-hidden and its menu items are hidden.
test.describe('App layout — responsive breakpoint transition', () => {
  test('sidebar collapses when the viewport crosses to a mobile width', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Desktop: the sidebar + its New Chat menu item are visible.
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(`${baseURL}/`)
    const sidebar = page.locator('#app-sidebar')
    await expect(sidebar).toBeVisible()
    const newChat = page.getByRole('menuitem', { name: /New Chat/ })
    await expect(newChat).toBeVisible({ timeout: 30000 })

    // Cross to a mobile width (< xs=480) → the sidebar auto-collapses
    // (aria-hidden) and its menu items are no longer visible.
    await page.setViewportSize({ width: 400, height: 800 })
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
    await expect(newChat).toBeHidden({ timeout: 10000 })
  })
})
