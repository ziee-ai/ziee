/**
 * Provider-button loading + error states E2E (audit gap all-5a77cd7a7676).
 *
 * social-login.spec.ts covers the *loaded* states of <ProviderButtons>
 * (buttons render, click navigates to /authorize, empty list renders
 * nothing). What it never exercised are the two NON-loaded branches in
 * ProviderButtons.tsx:48-70:
 *
 *   - `isLoading || !hasLoaded` → an antd <Spin> placeholder while the
 *     public `/api/auth/providers` list is still in flight.
 *   - `error` → an antd warning <Alert> ("Unable to load sign-in
 *     options") when that fetch fails.
 *
 * Both are driven through the REAL component: we only manipulate the
 * external HTTP boundary (the providers endpoint) — delaying the real
 * response to make the Spin observable, and failing it to drive the
 * Alert. The store's loading/error state machine and the component's
 * branch rendering all run for real (no mocks of the behavior itself).
 */
import { test, expect } from '../../fixtures/test-context'
import { getAdminToken, loginAsAdmin } from '../../common/auth-helpers'

/**
 * Enable the pre-seeded `google` row (migration 47) with stub creds so
 * it appears on the public `/api/auth/providers` list — mirrors the
 * helper in social-login.spec.ts.
 */
async function enableGoogleProvider(apiURL: string, adminToken: string) {
  const listRes = await fetch(`${apiURL}/api/admin/auth-providers`, {
    headers: { Authorization: `Bearer ${adminToken}` },
  })
  const list = (await listRes.json()) as Array<{ id: string; name: string }>
  const row = list.find(p => p.name === 'google')
  if (!row) throw new Error('Expected pre-seeded google row (migration 47)')
  const updRes = await fetch(`${apiURL}/api/admin/auth-providers/${row.id}`, {
    method: 'PUT',
    headers: {
      Authorization: `Bearer ${adminToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      enabled: true,
      config: {
        client_id: 'stub-client-id-for-e2e',
        client_secret: '',
        issuer_url: 'https://accounts.google.com',
        scopes: ['openid', 'email', 'profile'],
        display_name: 'Sign in with Google',
      },
    }),
  })
  if (!updRes.ok) {
    throw new Error(
      `Failed to enable google provider: ${updRes.status} ${await updRes.text()}`,
    )
  }
}

async function clearBrowserAuthState(page: any) {
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.context().clearCookies()
}

test.describe('Auth — provider-button loading + error states', () => {
  test('shows a spinner while the providers list is in flight', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await enableGoogleProvider(apiURL, adminToken)

    await clearBrowserAuthState(page)

    // Delay ONLY the upstream providers list so the store sits in its
    // isLoading state long enough to observe the <Spin>. The real
    // response (with the enabled google row) is then forwarded — so the
    // loaded branch renders the button afterwards.
    await page.route('**/api/auth/providers', async route => {
      await new Promise(resolve => setTimeout(resolve, 2500))
      await route.continue()
    })

    await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })

    // The login form renders immediately; the ProviderButtons region
    // shows the spinner while the (delayed) providers fetch is pending.
    await expect(page.getByLabel('Username or Email')).toBeVisible({
      timeout: 30_000,
    })
    await expect(page.locator('.ant-spin').first()).toBeVisible({
      timeout: 5_000,
    })

    // Once the delayed response lands, the loaded branch renders the
    // real provider button — proving the loading→loaded transition.
    await expect(
      page.getByRole('button', { name: /sign in with google/i }),
    ).toBeVisible({ timeout: 10_000 })
  })

  test('shows the warning Alert when the providers fetch fails', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await clearBrowserAuthState(page)

    // Fail the upstream providers endpoint → the store records `error`
    // → ProviderButtons renders the "Unable to load sign-in options"
    // warning Alert (its error branch).
    await page.route('**/api/auth/providers', route =>
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'upstream boom' }),
      }),
    )

    await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })

    await expect(page.getByLabel('Username or Email')).toBeVisible({
      timeout: 30_000,
    })
    await expect(
      page.getByText(/unable to load sign-in options/i),
    ).toBeVisible({ timeout: 10_000 })
  })
})
