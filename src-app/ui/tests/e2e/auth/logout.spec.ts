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
})
