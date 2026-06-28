/**
 * Parity test — one full end-to-end social-login flow against a real
 * navikt mock OAuth server (Docker). Catches integration regressions
 * the request-boundary mocks in `social-login.spec.ts` can't.
 *
 * Flow:
 *   1. Start navikt mock in Docker
 *   2. Login as admin
 *   3. Seed an `oidc` provider via the admin API, pointing at the mock
 *   4. Log out, visit /login
 *   5. Click "Sign in with NaviKt"
 *   6. Mock's /authorize page appears — submit `username` field
 *   7. Browser bounces through OUR /callback → /auth/callback#token=...
 *   8. AuthCallbackPage hydrates auth store + navigates home
 *   9. Verify logged-in landing
 *
 * Requires Docker. Skips with a clear message if Docker isn't on PATH.
 */
import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import { startNaviktMock } from '../../common/navikt-mock'

test.describe('Social login — navikt end-to-end parity', () => {
  test('full OIDC flow against navikt mock-oauth2-server', async ({
    page,
    testInfra,
  }, testInfo) => {
    let mock: Awaited<ReturnType<typeof startNaviktMock>>
    try {
      mock = await startNaviktMock()
    } catch (e: any) {
      testInfo.skip(true, `navikt mock unavailable: ${e?.message ?? e}`)
      return
    }
    try {
      const { baseURL, apiURL } = testInfra
      await loginAsAdmin(page, baseURL)
      const adminToken = await getAdminToken(apiURL)

      // Create a fresh OIDC provider pointing at the mock. Unique
      // name to avoid colliding with migration-47 pre-seeds + with
      // parallel test workers.
      const providerName = `navikt-${Date.now()}`
      const createRes = await fetch(`${apiURL}/api/admin/auth-providers`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${adminToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          name: providerName,
          provider_type: 'oidc',
          enabled: true,
          config: {
            client_id: 'navikt-e2e-client',
            client_secret: 'navikt-e2e-secret',
            issuer_url: mock.issuerUrl,
            scopes: ['openid', 'email', 'profile'],
            attribute_mapping: {
              user_id: 'sub',
              username: 'sub',
              // Map email to the real `email` claim (asserted verified via
              // the mock's claims field below). Our callback drops any email
              // the IdP didn't mark verified, so mapping email→sub (which the
              // mock never marks verified) would leave the new user with no
              // email and the create would be rejected.
              email: 'email',
              display_name: 'sub',
            },
            display_name: 'Sign in with NaviKt',
          },
        }),
      })
      if (!createRes.ok) {
        throw new Error(
          `provider create failed: ${createRes.status} ${await createRes.text()}`,
        )
      }

      // Log out + go to /login as a fresh user.
      await page.context().clearCookies()
      await page.evaluate(() => {
        localStorage.clear()
        sessionStorage.clear()
      })
      await page.goto(`${baseURL}/auth`, { waitUntil: 'domcontentloaded' })
      await page.getByLabel('Username or Email').waitFor({ timeout: 30_000 })

      // The provider button should be rendered (public endpoint
      // includes our new enabled row).
      const button = page.getByRole('button', { name: /sign in with navikt/i })
      await expect(button).toBeVisible({ timeout: 10_000 })

      // Click triggers full-page navigation → backend /authorize →
      // 307 to navikt /authorize. Navikt's UI is a tiny form; submit
      // a `username` field which becomes the `sub` claim.
      await button.click()

      // Wait for navikt's authorize page. It includes the word "Login"
      // in the form heading by default.
      await page.waitForURL(url => url.toString().includes(mock.baseUrl), {
        timeout: 15_000,
      })
      await page.locator('input[name="username"]').fill('e2e-navikt-user')
      // The default mock token carries `sub` but no email / email_verified.
      // The mock's `claims` textarea merges arbitrary claims into the issued
      // ID token, so assert a verified email — exactly what a real OIDC
      // provider returns and what our callback requires to provision.
      await page
        .locator('textarea[name="claims"], #claims')
        .first()
        .fill(
          '{"email":"navikt-e2e-user@example.com","email_verified":true}',
        )
      // Navikt's submit button is labelled "Sign-in" (with hyphen).
      await page.getByRole('button', { name: /sign-?in/i }).click()

      // Bounce: navikt → our /callback (with code) → AuthCallbackPage
      // (#token=...&return_to=...) → AuthCallbackPage scrubs + navigates
      // to "/" (the default return_to since we hit /login without one).
      await page.waitForURL(`${baseURL}/`, { timeout: 30_000 })

      // Logged-in landing: the auth store has a token + a user. Verify
      // localStorage carries the JWT (proves Auth.store hydrated).
      const tokenInStore = await page.evaluate(() => {
        const raw = localStorage.getItem('auth-storage')
        return raw ? JSON.parse(raw).state?.token : null
      })
      expect(tokenInStore).toBeTruthy()
      expect(tokenInStore).not.toBe('')

      // Cross-subsystem: the OAuth-derived session must be FUNCTIONAL for chat.
      // The existing social-login coverage stops at the logged-in landing; here
      // we navigate to the chat surface (the '/chat' new-chat page) and assert
      // the composer's send affordance renders for the OAuth-authenticated user.
      await page.goto(`${baseURL}/chat`, { waitUntil: 'domcontentloaded' })
      await expect(
        page.getByRole('button', { name: 'Send message' }),
      ).toBeVisible({ timeout: 30_000 })
    } finally {
      await mock!.stop()
    }
  })
})
