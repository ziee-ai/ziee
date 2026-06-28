import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-cd9dfa4076b4 — no E2E performed a SEQUENCE of layout interactions
// (sidebar + drawer + navigation + responsive). This drives one combined flow:
// open an edit drawer, close it, navigate, then on mobile open the sidebar and
// Escape-close it — exercising the interactions together rather than in
// isolation.
test.describe('App layout — combined multi-interaction flow', () => {
  test('drawer → navigate → mobile sidebar open + Escape', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Desktop: the app shell + sidebar are present.
    await page.setViewportSize({ width: 1280, height: 900 })
    await page.goto(`${baseURL}/settings/auth-providers`)
    await expect(page.locator('#app-sidebar')).toBeVisible({ timeout: 30000 })

    // 1) Open a drawer (edit the seeded google provider) then close it.
    await page.getByRole('button', { name: 'Edit google' }).click()
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await page.getByRole('button', { name: /^Cancel$/ }).click()
    await expect(drawer).toHaveCount(0, { timeout: 10000 })

    // 2) Navigate to another settings route.
    await page.goto(`${baseURL}/settings/about`)
    await expect(page.getByRole('heading', { name: 'About' })).toBeVisible({ timeout: 30000 })

    // 3) Switch to mobile: the sidebar auto-collapses; open it then Escape-close.
    await page.setViewportSize({ width: 400, height: 800 })
    const sidebar = page.locator('#app-sidebar')
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
    await page.getByRole('button', { name: 'Open navigation menu' }).click()
    await expect(sidebar).not.toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
    await page.keyboard.press('Escape')
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
  })
})
