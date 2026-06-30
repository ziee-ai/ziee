import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

// audit id all-eb2b4abb33fd — responsive/mobile breakpoint transitions were
// untested (0 of the E2E specs cross a breakpoint). The app sidebar
// (app-sidebar) auto-collapses when the viewport crosses into the `xs` mobile
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
    const sidebar = byTestId(page, 'app-sidebar')
    await expect(sidebar).toBeVisible()
    const newChat = byTestId(
      page,
      'layout-sidebar-primary-actions-menu-item-new-chat',
    )
    await expect(newChat).toBeVisible({ timeout: 30000 })

    // Cross to a mobile width (< xs=480) → the sidebar auto-collapses: it
    // slides off-screen via `translateX(-100%)` and is marked aria-hidden
    // (AppLayout.tsx). The menu items stay in the DOM (so crossing back
    // restores them without a remount) but are pushed out of the viewport, so
    // they're invisible to both sighted users and assistive tech. `toBeHidden`
    // is the wrong matcher here — it checks CSS display/visibility, not a
    // transform offset — so assert the item is OUT of the viewport instead.
    await page.setViewportSize({ width: 400, height: 800 })
    await expect(sidebar).toHaveAttribute('aria-hidden', 'true', { timeout: 10000 })
    await expect(newChat).not.toBeInViewport({ timeout: 10000 })
  })
})
