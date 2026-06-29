import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, clearAuthState } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — unauthenticated access to a protected route renders the AuthPage.
 *
 * `AuthGuard` (multi-user mode) renders `<AuthPage />` when `!isAuthenticated`,
 * regardless of the requested path. This asserts a deep-link to a protected
 * settings route while logged out shows the login form instead of the page.
 */

test.describe('Authentication — protected route guard', () => {
  test('visiting a protected route while logged out shows the login form', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // An admin must exist (otherwise the guard routes to /setup). Create one,
    // then drop the auth state so we're unauthenticated but past setup.
    await loginAsAdmin(page, baseURL)
    await clearAuthState(page)

    // Deep-link to a protected route.
    await page.goto(`${baseURL}/settings/profile`, { waitUntil: 'load' })

    // The guard renders the AuthPage login form rather than the profile page.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 30000 })
  })
})
