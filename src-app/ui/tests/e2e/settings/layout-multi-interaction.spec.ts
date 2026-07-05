import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

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
    await expect(byTestId(page, 'app-sidebar')).toBeVisible({ timeout: 30000 })

    // 1) Open a drawer (edit the seeded google provider) then close it. The
    //    drawer form is present only while the drawer is open (destroyOnHidden).
    await byTestId(page, 'authprov-edit-button-google').click()
    const drawerForm = byTestId(page, 'authprov-drawer-form')
    await expect(drawerForm).toBeVisible({ timeout: 10000 })
    await byTestId(page, 'authprov-drawer-cancel-button').click()
    await expect(drawerForm).toHaveCount(0, { timeout: 10000 })

    // 2) Navigate to another settings route.
    await page.goto(`${baseURL}/settings/about`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({ timeout: 30000 })

    // 3) Switch to mobile: the sidebar becomes a closed Sheet (content not
    //    mounted); open it via the toggle then Escape-close it.
    await page.setViewportSize({ width: 400, height: 800 })
    const sidebar = byTestId(page, 'app-sidebar')
    await expect(sidebar).toBeHidden({ timeout: 10000 })
    await byTestId(page, 'layout-sidebar-toggle-button').click()
    await expect(sidebar).toBeVisible({ timeout: 10000 })
    await page.keyboard.press('Escape')
    await expect(sidebar).toBeHidden({ timeout: 10000 })
  })
})
