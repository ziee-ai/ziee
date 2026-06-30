import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the collapsed-sidebar state survives client-side navigation.
 *
 * Audit gap: AppLayout.store holds `isSidebarCollapsed` at module scope
 * (deliberately NOT a per-mount useRef, so a route change doesn't reset it
 * — see the store comment), but no test navigated to prove it. This
 * collapses the sidebar, then navigates to another route via in-app routing
 * (not a reload) and asserts the sidebar is still collapsed.
 */

test.describe('Layout — sidebar collapse survives navigation', () => {
  test('collapsing then navigating keeps the sidebar collapsed', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Land on an authed route with the app shell (NOT profile — we navigate
    // to profile below to cross a route boundary).
    await page.goto(`${baseURL}/settings/hardware`)
    await byTestId(page, 'settings-page-title').waitFor({
      state: 'visible',
      timeout: 30000,
    })

    // Collapse the sidebar (the toggle button starts in the expanded/"Close"
    // state; clicking it collapses, flipping its aria-label to "Open").
    const toggle = byTestId(page, 'layout-sidebar-toggle-button')
    await expect(toggle).toHaveAttribute('aria-label', 'Close navigation menu')
    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-label', 'Open navigation menu')

    // Client-side navigate via the profile widget dropdown (NOT a reload).
    // The sidebar-collapse animation keeps the widget moving briefly, so the
    // default stability gate can time out — force the click (already visible).
    await byTestId(page, 'user-profile-widget').click({ force: true })
    await byTestId(page, 'userprofile-menu-dropdown-item-profile').click()
    await expect(page).toHaveURL(/\/settings\/profile$/)

    // The collapsed state persisted across the route change (still showing
    // the "Open navigation menu" affordance, never reset to expanded).
    await expect(
      byTestId(page, 'layout-sidebar-toggle-button'),
    ).toHaveAttribute('aria-label', 'Open navigation menu')
  })
})
