import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the sidebar UserProfileWidget collapse/expand rendering
 * (audit gap all-34ec884b0091).
 *
 * UserProfileWidget.tsx renders the logged-in user's name in the sidebar
 * footer. When the sidebar collapses (Stores.AppLayout.isSidebarCollapsed,
 * toggled by SidebarToggleButton), the username label is animated away
 * (`opacity: 0; max-width: 0`) and restored on expand. The toggle's
 * disabled/icon state was exercised elsewhere, but the widget's own
 * collapsed-vs-expanded rendering response was never asserted. This drives
 * the real toggle button and asserts the username label hides + re-shows.
 *
 * The username `<span>` carries `title={user.username}` and animates
 * opacity/max-width — we assert the computed CSS (deterministic; toHaveCSS
 * auto-retries past the 200ms transition) rather than Playwright's
 * visibility heuristic.
 */

test.describe('Sidebar — UserProfileWidget collapse/expand', () => {
  test('toggling the sidebar hides then restores the username label', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Default 1280px viewport keeps the sidebar expanded on mount.
    await loginAsAdmin(page, baseURL)

    // The widget + its username label (admin is the default login user).
    const widget = page.getByTestId('user-profile-widget')
    await expect(widget).toBeVisible({ timeout: 30_000 })
    const label = widget.locator('span[title="admin"]')

    // The sidebar collapse toggle (aria-controls ties it to #app-sidebar).
    const toggle = page.locator('button[aria-controls="app-sidebar"]')
    await expect(toggle).toBeVisible()

    // Normalize to the expanded state (responsive logic may have collapsed
    // it on a narrow CI viewport).
    if ((await toggle.getAttribute('aria-expanded')) === 'false') {
      await toggle.click()
      await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    }

    // EXPANDED: the username label is fully shown.
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await expect(label).toHaveCSS('opacity', '1')
    await expect(label).toHaveCSS('max-width', '200px')

    // COLLAPSE: click the toggle → label animates to hidden.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await expect(label).toHaveCSS('opacity', '0')
    await expect(label).toHaveCSS('max-width', '0px')

    // EXPAND again: label is restored — proves the toggle is a real two-way
    // control over the widget's rendering, not a one-shot.
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await expect(label).toHaveCSS('opacity', '1')
    await expect(label).toHaveCSS('max-width', '200px')
  })
})
