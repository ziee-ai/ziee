import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, clearAuthState } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// ---------------------------------------------------------------------------
// The Settings → About page route (`/settings/about`) is registered with
// `requiresAuth: true` (server-update/module.tsx). The existing server-update
// specs only ever reach it as an authenticated admin; the unauthenticated
// access path — AuthGuard bouncing an anonymous visitor to /auth — was never
// verified end-to-end. This pins that gate.
// ---------------------------------------------------------------------------

test.describe('About page — unauthenticated access', () => {
  test('an anonymous visitor to /settings/about is bounced to /auth', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Establish an origin so clearAuthState's localStorage.clear() has a
    // document to act on, then wipe ALL client auth state (localStorage,
    // sessionStorage, cookies incl. the HTTP-only refresh cookie).
    await page.goto(baseURL)
    await clearAuthState(page)

    // Deep-link the protected About route while unauthenticated.
    await page.goto(`${baseURL}/settings/about`)

    // AuthGuard redirects any protected route to /auth when isAuthenticated
    // is false, and the login form renders.
    await page.waitForURL(/\/auth/, { timeout: 30000 })
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })

    // The About content must NOT be exposed to the anonymous visitor.
    await expect(byTestId(page, 'serverupd-about-card')).toHaveCount(0)
  })

  test('positive control: an authenticated admin reaches the About page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.route(/\/api\/server-update\/status$/, async route => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          current_version: '0.1.0',
          latest_version: '0.1.0',
          update_available: false,
          release_url: null,
          notes: null,
          checked_at: '2026-06-12T00:00:00Z',
          enabled: true,
        }),
      })
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)

    // No redirect to /auth, and the About content renders.
    await expect(page).not.toHaveURL(/\/auth/)
    await expect(byTestId(page, 'serverupd-about-card')).toBeVisible({ timeout: 30000 })
  })
})
