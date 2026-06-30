import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — logging out via the sidebar UserProfileWidget dropdown
 * (audit gap all-0f545d2eb300).
 *
 * `UserProfileWidget.tsx` renders a Dropdown (trigger
 * `[data-testid="user-profile-widget"]`) whose "Logout" menu item calls
 * `Stores.Auth.logoutUser()`, which clears the persisted token/user and
 * flips `isAuthenticated=false`. The existing profile spec only exercises
 * the dropdown's "Profile" item — the logout path (the actual sign-out
 * effect) had no E2E. This drives the real control and asserts the token
 * is cleared AND the app falls back to the unauthenticated `/auth` page
 * (no mocks — the real logout endpoint + AuthGuard run).
 */

test.describe('Auth — logout via the sidebar profile widget', () => {
  test('clicking Logout signs the user out and clears the token', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Sanity: we are authenticated (a persisted token exists).
    const tokenBefore = await page.evaluate(() => {
      const raw = localStorage.getItem('auth-storage')
      return raw ? JSON.parse(raw).state?.token : null
    })
    expect(tokenBefore, 'should be logged in before logout').toBeTruthy()

    // Open the sidebar profile widget dropdown and click Logout.
    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()

    // The app drops to the unauthenticated surface: once isAuthenticated flips
    // to false the AuthGuard renders the login wall INLINE (it does not change
    // the URL), so assert the login form appears rather than a /auth redirect.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({
      timeout: 30000,
    })

    // The persisted token is cleared (logoutUser nulls state.token).
    await expect
      .poll(
        () =>
          page.evaluate(() => {
            const raw = localStorage.getItem('auth-storage')
            if (!raw) return null
            try {
              return JSON.parse(raw).state?.token ?? null
            } catch {
              return null
            }
          }),
        { timeout: 15000 },
      )
      .toBeNull()

    // The profile widget is gone now that there's no user.
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)
  })
})
