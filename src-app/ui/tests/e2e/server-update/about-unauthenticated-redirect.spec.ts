import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken, clearAuthState } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// ---------------------------------------------------------------------------
// The Settings → About page route (`/settings/about`) is registered with
// `requiresAuth: true` (server-update/module.tsx). The existing server-update
// specs only ever reach it as an authenticated admin; the unauthenticated
// access path — AuthGuard failing closed to the login wall (rendered in place,
// URL preserved) — was never verified end-to-end. This pins that gate.
// ---------------------------------------------------------------------------

test.describe('About page — unauthenticated access', () => {
  test('an anonymous visitor to /settings/about hits the login wall', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Complete setup first (create the admin) via the API — NOT a browser
    // login. On a fresh backend with no admin, the setup-incomplete gate takes
    // precedence and sends ANY visitor to /setup (not /auth). getAdminToken
    // runs over `fetch`, so it establishes NO session cookie on `page`'s
    // context — the visitor stays genuinely unauthenticated (a browser login
    // would leave an HTTP-only refresh cookie that silently re-authenticates).
    await getAdminToken(apiURL)

    // Establish an origin, then wipe any client auth state (belt-and-suspenders).
    await page.goto(baseURL)
    await clearAuthState(page)

    // Deep-link the protected About route while unauthenticated.
    await page.goto(`${baseURL}/settings/about`)

    // AuthGuard FAILS CLOSED by rendering the login wall IN PLACE (the URL is
    // preserved, not redirected to /auth — same pattern as auth-guard-fails-
    // closed). Assert the login form shows and the protected content does not.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 30000 })

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
