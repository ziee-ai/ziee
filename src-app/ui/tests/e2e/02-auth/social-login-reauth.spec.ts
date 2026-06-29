/**
 * Social login → edit profile (username change) → logout → re-auth.
 *
 * Verifies that after a first OIDC login the user can rename themselves, and a
 * SECOND login with the SAME external identity (`sub`) lands them back in the
 * SAME account with the EDITED username intact — the callback resolves an
 * existing auth-link by external id and does NOT re-sync the username from the
 * IdP (so a local rename is durable). Extends the navikt parity flow; Docker-
 * gated (skips cleanly without it).
 */
import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'
import { startNaviktMock } from '../../common/navikt-mock'
import { byTestId } from '../testid'

async function naviktLogin(
  page: Page,
  baseURL: string,
  mockBaseUrl: string,
  providerName: string,
  sub: string,
  email: string,
) {
  await page.context().clearCookies()
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.goto(`${baseURL}/auth`, { waitUntil: 'domcontentloaded' })
  await byTestId(page, 'auth-login-username').waitFor({ timeout: 30_000 })

  const providerBtn = byTestId(page, `auth-provider-btn-${providerName}`)
  await expect(providerBtn).toBeVisible({ timeout: 10_000 })
  await providerBtn.click()

  // The mock OIDC server's login page is external (not our kit) — select on
  // its form attributes.
  await page.waitForURL((url) => url.toString().includes(mockBaseUrl), {
    timeout: 15_000,
  })
  await page.locator('input[name="username"]').fill(sub)
  await page
    .locator('textarea[name="claims"], #claims')
    .first()
    .fill(JSON.stringify({ email, email_verified: true }))
  await page.locator('button[type="submit"]').first().click()
  await page.waitForURL(`${baseURL}/`, { timeout: 30_000 })
}

test.describe('Social login — re-auth after profile rename', () => {
  test('rename persists across a second OIDC login with the same identity', async ({
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

      const providerName = `navikt-reauth-${Date.now()}`
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

      const sub = `reauth-user-${Date.now()}`
      const email = `${sub}@example.com`
      const renamed = `${sub}-renamed`

      // First login → provisions the account (username = sub).
      await naviktLogin(page, baseURL, mock.baseUrl, providerName, sub, email)

      // Rename the username on the profile page.
      await page.goto(`${baseURL}/settings/profile`)
      await byTestId(page, 'profile-username-input').waitFor({ timeout: 30_000 })
      await byTestId(page, 'profile-username-input').fill(renamed)
      const savePromise = page.waitForResponse(
        r => r.url().includes('/api/auth/profile') && r.request().method() === 'POST',
        { timeout: 15_000 },
      )
      await byTestId(page, 'profile-save-button').click()
      expect((await savePromise).ok()).toBeTruthy()

      // Log out + log in AGAIN with the same external identity (sub).
      await naviktLogin(page, baseURL, mock.baseUrl, providerName, sub, email)

      // Re-auth reached the SAME account and the rename is intact (the callback
      // did not overwrite the local username from the IdP sub).
      await page.goto(`${baseURL}/settings/profile`)
      await byTestId(page, 'profile-username-input').waitFor({ timeout: 30_000 })
      await expect(byTestId(page, 'profile-username-input')).toHaveValue(renamed)
    } finally {
      await mock!.stop()
    }
  })
})
