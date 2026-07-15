import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — logout via the UserProfileWidget dropdown.
 *
 * `UserProfileWidget` renders a Dropdown (trigger `[data-testid=
 * "user-profile-widget"]`) whose "Logout" item calls `Stores.Auth.logoutUser()`.
 * After logout the AuthGuard renders the AuthPage. This drives the real
 * click-path (no API shortcut) and asserts the user lands back on the login form.
 */

test.describe('Authentication — logout', () => {
  test('logging out via the profile dropdown returns to the login form', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // Open the profile dropdown in the sidebar footer.
    const widget = byTestId(page, 'user-profile-widget')
    await expect(widget).toBeVisible({ timeout: 30000 })
    await widget.click()

    // Click the "Logout" menu item.
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()

    // The AuthPage login form replaces the app shell.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    // The authenticated sidebar widget is gone.
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)
  })
  // The stored token must be gone, not just the rendered UI: a reloaded tab
  // rehydrates `token` from this key, so a surviving value would resurrect the
  // session.
  test('logout clears the persisted token', async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({
      timeout: 15000,
    })

    const persisted = await page.evaluate(() =>
      localStorage.getItem('auth-storage'),
    )
    expect(persisted, 'auth-storage should still exist').toBeTruthy()
    expect(JSON.parse(persisted as string)?.state?.token).toBeNull()
  })

  // The SERVER-SIDE backstop, isolated from the cross-tab sync signal.
  //
  // With the SSE stream blocked, tab 2 gets NO notification that tab 1 logged
  // out — exactly the "SSE is down / tab is idle" residual. It must still be
  // unable to act: its access token is revoked server-side, so its next real
  // request 401s and the store tears down. This is what makes the SSE signal an
  // optimisation rather than the security boundary (and why no BroadcastChannel
  // is needed).
  test('a tab with no sync stream is still dead after another tab logs out', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const tab2 = await page.context().newPage()
    try {
      // Block the SSE subscribe BEFORE navigating, so tab 2 never receives the
      // Session signal.
      await tab2.route('**/api/sync/subscribe*', route => route.abort())
      await tab2.goto(`${baseURL}/`)
      await tab2.waitForLoadState('load')
      await expect(byTestId(tab2, 'user-profile-widget')).toBeVisible({
        timeout: 30000,
      })

      // Tab 1 logs out.
      await byTestId(page, 'user-profile-widget').click()
      await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()
      await expect(byTestId(page, 'auth-login-username')).toBeVisible({
        timeout: 15000,
      })

      // Tab 2 makes a real request (navigating to settings forces /auth/me +
      // data loads). The revoked token 401s -> refresh 401s -> teardown.
      await tab2.goto(`${baseURL}/settings`)
      await tab2.waitForLoadState('load')
      await expect(byTestId(tab2, 'auth-login-username')).toBeVisible({
        timeout: 30000,
      })
    } finally {
      await tab2.close()
    }
  })
})
