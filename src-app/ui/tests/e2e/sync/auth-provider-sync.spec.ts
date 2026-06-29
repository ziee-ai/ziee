import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-b004ccdaf6f7 — realtime sync for the AuthProvider entity. The
// admin auth-providers handlers publish AuthProvider/{Create,Update,Delete} to
// every auth_providers::read holder (handlers.rs:1725,1808,1881,1940) so other
// devices' AuthProvidersAdmin store (subscribed to sync:auth_provider) refetch
// WITHOUT a reload. REAL cross-device test against the live SSE channel.
//
// Run with --workers=1.

async function gotoAuthProviders(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/auth-providers`)
  await page.waitForLoadState('load')
  // The seeded google row's toggle is the stable "page mounted + store loaded"
  // signal (migration 47 pre-seeds google/microsoft/apple).
  await expect(
    page.getByTestId(/^authprov-row-/).filter({ hasText: 'google' }).getByTestId(/^authprov-toggle-switch-/),
  ).toBeVisible({ timeout: 30_000 })
}

test.describe('Realtime sync — auth_provider', () => {
  test('creating a provider on device A appears on device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await gotoAuthProviders(page, baseURL)

    const name = `sync-oidc-${Date.now()}`

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoAuthProviders(pageB, baseURL)

      // Sanity: device B does not yet show the soon-to-be-created provider.
      await expect(
        pageB.getByTestId(/^authprov-row-/).filter({ hasText: name }).getByTestId(/^authprov-toggle-switch-/),
      ).toHaveCount(0)

      // Device A creates a new OIDC provider via REST (enabled:false → no probe)
      // → publishes AuthProvider/Create.
      const createRes = await page.request.post(
        `${baseURL}/api/admin/auth-providers`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: {
            name,
            provider_type: 'oidc',
            enabled: false,
            config: {
              client_id: 'sync-e2e-client',
              client_secret: 'sync-e2e-secret',
              issuer_url: 'https://idp.invalid/oidc',
              scopes: ['openid', 'email', 'profile'],
            },
          },
        },
      )
      expect(
        createRes.ok(),
        `create failed: ${createRes.status()} ${await createRes.text()}`,
      ).toBeTruthy()

      // Device B's list must gain the new provider's toggle WITHOUT a reload.
      await expect(
        pageB.getByTestId(/^authprov-row-/).filter({ hasText: name }).getByTestId(/^authprov-toggle-switch-/),
      ).toBeVisible({ timeout: 45_000 })
    } finally {
      await ctxB.close()
    }
  })
})
