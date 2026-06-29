import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the SidebarToggleButton's own click → state change
 * (audit gap b6826346c6ac).
 *
 * `SidebarToggleButton.tsx` renders an antd Button whose `onClick` calls
 * `Stores.AppLayout.toggleSidebar()`. Its `aria-label` flips between
 * "Close navigation menu" (expanded) and "Open navigation menu"
 * (collapsed), `aria-expanded` mirrors `!isSidebarCollapsed`, and
 * `aria-controls="app-sidebar"` ties it to the sidebar element which
 * collapses by sliding off-screen-left via `transform: translateX(-100%)`
 * (AppLayout.tsx:422).
 *
 * The sibling `user-profile-widget-collapse.spec.ts` asserts `aria-expanded`
 * + the UserProfileWidget label animation, but never the button's OWN
 * `aria-label` text nor the `#app-sidebar` element physically translating
 * off-screen. This drives the toggle button and asserts BOTH of those, both
 * ways — proving the button is a real two-way control over the sidebar's
 * collapsed state, not just a label-animation trigger.
 *
 * Default 1280px viewport keeps the layout on the desktop (non-xs) branch,
 * where collapse = `translateX(-100%)` (the sidebar's bounding box x goes
 * strongly negative as it slides left), distinct from the xs drawer branch.
 */

test.describe('Sidebar — toggle button click + state change', () => {
  test('clicking the toggle flips its aria state + slides the sidebar off-screen, both ways', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const toggle = page.locator('button[aria-controls="app-sidebar"]')
    const sidebar = page.locator('#app-sidebar')
    await expect(toggle).toBeVisible({ timeout: 30_000 })
    await expect(sidebar).toBeVisible()

    // Normalize to the EXPANDED state (a narrow CI viewport's responsive
    // logic may have collapsed it on mount).
    if ((await toggle.getAttribute('aria-expanded')) === 'false') {
      await toggle.click()
      await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    }

    // EXPANDED: button advertises "close", sidebar sits at the left edge.
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await expect(toggle).toHaveAttribute('aria-label', 'Close navigation menu')
    await expect
      .poll(async () => (await sidebar.boundingBox())?.x ?? null, {
        timeout: 5_000,
      })
      .toBeGreaterThan(-5) // on-screen at x ≈ 0

    // COLLAPSE: one click → aria flips to "open"/expanded=false AND the
    // sidebar element translates fully off-screen to the left.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await expect(toggle).toHaveAttribute('aria-label', 'Open navigation menu')
    await expect
      .poll(async () => (await sidebar.boundingBox())?.x ?? 0, {
        timeout: 5_000,
      })
      .toBeLessThan(-50) // slid off-screen-left via translateX(-100%)

    // EXPAND again: a second click restores both the aria state and the
    // sidebar's on-screen position — proves it's a real two-way toggle.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await expect(toggle).toHaveAttribute('aria-label', 'Close navigation menu')
    await expect
      .poll(async () => (await sidebar.boundingBox())?.x ?? null, {
        timeout: 5_000,
      })
      .toBeGreaterThan(-5)
  })
})
