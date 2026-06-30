import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, clearAuthState } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — public vs protected route separation (RouterComponent.tsx groups
 * routes by `requiresAuth`; protected routes are wrapped in the auth guard
 * → `<Navigate to="/auth" />` when unauthenticated).
 *
 * Audit gap: this separation was untested. An unauthenticated user hitting a
 * protected route is redirected to /auth; the public /auth route renders
 * without auth.
 */

test.describe('Routing — public vs protected', () => {
  test('an unauthenticated user hitting a protected route is sent to /auth', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Ensure an admin exists (so the app is past first-run setup), then drop
    // the session so we are unauthenticated.
    await loginAsAdmin(page, baseURL)
    await clearAuthState(page)

    await page.goto(`${baseURL}/settings/profile`)

    // Fail closed: the AuthGuard renders the login wall INLINE (preserving the
    // deep-link URL rather than redirecting to /auth), and the protected
    // content must NOT render.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'profile-display-name-input')).toHaveCount(0)
  })

  test('the public /auth route renders without authentication', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await clearAuthState(page)

    await page.goto(`${baseURL}/auth`)
    await expect(page).toHaveURL(/\/auth/)
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
  })
})
