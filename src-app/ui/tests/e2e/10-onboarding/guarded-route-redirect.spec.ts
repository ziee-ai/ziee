import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
} from '../../common/auth-helpers'

/**
 * E2E — OnboardingRedirect bounces a not-yet-onboarded, non-admin user to
 * /onboarding even when they DEEP-LINK into a guarded route.
 *
 * OnboardingRedirect.tsx:39-51 runs as a routerEffect sibling of <Routes>:
 * once auth settles, a non-admin with an incomplete guide is navigated to
 * the first incomplete guide regardless of which guarded path they hit.
 * The existing onboarding-wizard spec only covers landing on `/`; this
 * asserts the redirect fires on an unrelated guarded deep-link.
 */

async function injectTokenAndVisit(
  page: import('@playwright/test').Page,
  baseURL: string,
  username: string,
  target: string,
) {
  const res = await fetch(`${baseURL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password: 'password123' }),
  })
  if (!res.ok) {
    throw new Error(`login failed: ${res.status} ${await res.text()}`)
  }
  const { access_token } = await res.json()
  // NOTE: deliberately do NOT complete onboarding — that's the whole point.
  await page.addInitScript(token => {
    try {
      localStorage.setItem(
        'auth-storage',
        JSON.stringify({ state: { token }, version: 0 }),
      )
    } catch {
      /* ignore */
    }
  }, access_token)
  await page.goto(`${baseURL}${target}`)
}

test.describe('Onboarding — guarded-route redirect', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('deep-linking a guarded route redirects to /onboarding', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `guarded_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )

    // Hit a guarded settings deep-link, NOT the home route.
    await injectTokenAndVisit(page, baseURL, username, '/settings/profile')

    // AuthGuard authenticates → OnboardingRedirect bounces to the wizard.
    await page.waitForURL(/\/onboarding/, { timeout: 15000 })
    await expect(page).toHaveURL(/\/onboarding/)
  })
})
