import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
} from '../../common/auth-helpers'

/**
 * E2E — OnboardingRedirect ↔ AuthGuard *suppression* conditions.
 *
 * The redirect-FIRES direction (a not-yet-onboarded non-admin bounced to
 * /onboarding on a guarded deep-link) is already covered by
 * `guarded-route-redirect.spec.ts`. This spec pins the complementary half —
 * the two `OnboardingRedirect.tsx:45-52` skip conditions that must NOT
 * redirect, so the interaction is proven BOTH ways:
 *
 *   1. A non-admin who has COMPLETED every guide is left on the app route.
 *   2. An ADMIN (is_admin skip at line 47) is never bounced, even with
 *      onboarding incomplete — otherwise a remote/phone admin session would
 *      be trapped in a loop it can't escape.
 *
 * Without these, a regression that dropped the `loaded`/completion or the
 * admin guard would silently re-trap onboarded users / admins.
 */

const APP_ROUTE = '/settings/profile'

async function loginViaApi(
  baseURL: string,
  username: string,
): Promise<string> {
  const res = await fetch(`${baseURL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password: 'password123' }),
  })
  if (!res.ok) {
    throw new Error(`login failed: ${res.status} ${await res.text()}`)
  }
  const { access_token } = await res.json()
  return access_token as string
}

async function visitWithToken(
  page: import('@playwright/test').Page,
  baseURL: string,
  token: string,
  target: string,
): Promise<void> {
  await page.addInitScript(t => {
    try {
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token: t }, version: 0 }),
      )
    } catch {
      /* ignore */
    }
  }, token)
  await page.goto(`${baseURL}${target}`)
}

/**
 * Give OnboardingRedirect ample time to run its routerEffect, then assert it
 * did NOT bounce us to /onboarding (a redirect would have changed the URL).
 */
async function assertNotRedirected(
  page: import('@playwright/test').Page,
): Promise<void> {
  // The effect runs once auth + onboarding state settle. Don't wait for
  // 'networkidle' — the realtime sync SSE stream (/api/sync/subscribe) keeps a
  // request open indefinitely, so networkidle never fires and the test times
  // out. A fixed settle window is enough for any redirect to have fired.
  await page.waitForLoadState('load')
  await page.waitForTimeout(3000)
  await expect(page).not.toHaveURL(/\/onboarding/)
}

test.describe('Onboarding — AuthGuard redirect suppression', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('a fully-onboarded non-admin is NOT redirected to /onboarding', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `onboarded_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )

    const userToken = await loginViaApi(baseURL, username)

    // Mark the (only) registered guide complete via the real REST endpoint —
    // this is what populates completedGuideIds, the suppression input.
    const complete = await fetch(
      `${apiURL}/api/onboarding/getting-started/complete`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${userToken}` },
      },
    )
    expect(
      complete.status,
      `guide-complete should succeed: ${complete.status}`,
    ).toBeLessThan(400)

    // Navigate to a guarded app route. A NON-onboarded user would be bounced
    // (see guarded-route-redirect.spec.ts); this completed user must NOT be.
    await visitWithToken(page, baseURL, userToken, APP_ROUTE)
    await assertNotRedirected(page)
    await expect(page).toHaveURL(new RegExp(APP_ROUTE.replace('/', '\\/')))
  })

  test('an admin is NOT redirected even with onboarding incomplete', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    // Deliberately do NOT complete any guide for the admin — the is_admin
    // skip (OnboardingRedirect.tsx:47) must suppress the redirect regardless.
    await visitWithToken(page, baseURL, adminToken, APP_ROUTE)
    await assertNotRedirected(page)
    await expect(page).toHaveURL(new RegExp(APP_ROUTE.replace('/', '\\/')))
  })
})
