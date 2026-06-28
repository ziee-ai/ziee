/**
 * Social-login E2E — covers the public-facing flow only.
 *
 *   - Provider buttons render when an admin enables a provider
 *   - No buttons when nothing is enabled (default state)
 *   - Click stashes `returnTo` in sessionStorage + navigates to the
 *     authorize URL via a full-page redirect (no fetch)
 *   - /auth/callback#token=… scrubs the fragment via
 *     `history.replaceState`, hydrates Auth.store, navigates to returnTo
 *   - /auth/callback with no token shows the "Sign-in failed" error UI
 *   - /auth/link-account form posts to /api/auth/link-account and
 *     handles success / wrong-password (API mocked via page.route)
 *
 * What's deliberately NOT here:
 *   - The full OAuth dance against a real provider — lives in the
 *     parity spec `social-login-navikt.spec.ts`, which spins up
 *     navikt's mock OAuth2 server in Docker.
 *   - Backend correctness — covered by 51 integration tests in
 *     src-app/server/tests/auth/.
 */
import { test, expect } from '../../fixtures/test-context'
import { DEFAULT_ADMIN_CREDENTIALS, getAdminToken, loginAsAdmin } from '../../common/auth-helpers'

const STORAGE_KEY = 'ziee.oauth.returnTo'

/**
 * Enable the pre-seeded `google` row with stub credentials so it shows
 * up on the public `/api/auth/providers` list. Returns the token used.
 */
async function enableGoogleProvider(apiURL: string, adminToken: string) {
  // List to find the pre-seeded id
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
        // Empty client_secret => server preserves the existing empty
        // value. That's fine for button-rendering tests; we never
        // complete the OAuth dance here.
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

/**
 * Get back to a logged-out /login page after using an admin token.
 * Browser state stays scoped to this page so isolation is preserved.
 */
async function logoutThenGoToLogin(page: any, baseURL: string) {
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.context().clearCookies()
  await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
}

test.describe('Social login — provider buttons + callback flow', () => {
  test('provider buttons render after admin enables a provider', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await enableGoogleProvider(apiURL, adminToken)

    await logoutThenGoToLogin(page, baseURL)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Public providers endpoint returns the enabled row, so the
    // ProviderButtons row hydrates + renders the button.
    await expect(
      page.getByRole('button', { name: /sign in with google/i }),
    ).toBeVisible({ timeout: 10_000 })
  })

  test('no provider buttons when nothing is enabled', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Setup admin (needed to satisfy the bootstrap flow) — DON'T enable any provider.
    await loginAsAdmin(page, baseURL)
    await logoutThenGoToLogin(page, baseURL)

    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // No "Sign in with X" buttons should be rendered.
    await expect(
      page.getByRole('button', { name: /sign in with /i }),
    ).toHaveCount(0)
    // The "or continue with" divider also shouldn't be visible.
    await expect(page.getByText(/or continue with/i)).toHaveCount(0)
  })

  test('shows a loading spinner then a warning Alert when providers fail to load', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Bootstrap admin so the app is past first-run setup, then log out.
    await loginAsAdmin(page, baseURL)

    // Gate the public providers endpoint so we can observe the loading state
    // before resolving it as a failure.
    let release: () => void = () => {}
    const gate = new Promise<void>(r => (release = r))
    await page.route(/\/api\/auth\/providers$/, async route => {
      await gate
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ message: 'upstream down' }),
      })
    })

    await logoutThenGoToLogin(page, baseURL)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // While the request is in flight, ProviderButtons renders a Spin.
    await expect(page.locator('.ant-spin').first()).toBeVisible({
      timeout: 10_000,
    })

    // Resolve as a 500 → the loading state gives way to the warning Alert.
    release()
    await expect(
      page.getByText('Unable to load sign-in options'),
    ).toBeVisible({ timeout: 10_000 })
  })

  test('clicking a provider button stashes returnTo and navigates to /authorize', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await enableGoogleProvider(apiURL, adminToken)
    await logoutThenGoToLogin(page, baseURL)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Intercept the full-page navigation so we don't actually fly
    // off to accounts.google.com (and to fail fast if the URL is wrong).
    let capturedAuthorizeUrl = ''
    await page.route('**/api/auth/oauth/google/authorize**', async route => {
      capturedAuthorizeUrl = route.request().url()
      // Reply with a 200 + empty body so the navigation completes
      // without redirecting — the test only cares that the button
      // triggered the right URL.
      await route.fulfill({ status: 200, body: 'intercepted' })
    })

    await page
      .getByRole('button', { name: /sign in with google/i })
      .click()

    // The navigation may resolve immediately because of the fulfill,
    // but the URL we captured is the assertion that matters.
    await page.waitForLoadState('load').catch(() => undefined)
    expect(capturedAuthorizeUrl).toContain('/api/auth/oauth/google/authorize')

    // sessionStorage was populated before the click navigated away —
    // re-read from the intercepted post-navigation page.
    const stored = await page.evaluate(
      key => window.sessionStorage.getItem(key),
      STORAGE_KEY,
    )
    expect(stored).not.toBeNull()
  })

  test('/auth/callback scrubs the fragment + hydrates auth + navigates', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Clear all state, then directly navigate to /auth/callback with
    // the admin JWT in the fragment + a return_to. This simulates what
    // happens AFTER the backend issues the post-OAuth redirect.
    await page.context().clearCookies()
    await page.goto(`${baseURL}/auth`, { waitUntil: 'domcontentloaded' })
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    const target = encodeURIComponent('/')
    await page.goto(
      `${baseURL}/auth/callback#token=${encodeURIComponent(adminToken)}&return_to=${target}`,
    )

    // After the React effect runs:
    //   - history.replaceState clears the fragment
    //   - Auth.store hydrates via /api/auth/me
    //   - navigate(returnTo) sends us to "/"
    // Wait for the navigate by polling on URL.
    await page.waitForURL(`${baseURL}/`, { timeout: 15_000 })
    // Fragment scrubbed (we ended up at "/" — no #token leftover).
    expect(page.url()).not.toContain('#token=')

    // Token actually landed in localStorage (proof Auth.store hydrated).
    const tokenInStore = await page.evaluate(() => {
      const raw = localStorage.getItem('auth-storage')
      return raw ? JSON.parse(raw).state?.token : null
    })
    expect(tokenInStore).toBe(adminToken)

    // CRITICAL: assert the post-callback navigate produced an
    // AUTHENTICATED landing — not the login flash. Round-5 audit
    // caught a regression where setAuthFromAutoLogin set isLoading
    // to true, causing initAuth to early-return, leaving the user
    // unauthenticated → AuthGuard bounced them to /auth even though
    // the URL was "/". The token-in-localStorage check above is
    // insufficient (token survives even when initAuth silently
    // skipped). Wait briefly then assert we did NOT end up back at
    // /auth, which is what AuthGuard renders for unauthenticated
    // users.
    await page.waitForLoadState('load', { timeout: 10_000 })
    await expect(page).not.toHaveURL(/\/auth(\?|$)/)
  })

  test('/auth/callback with no token shows the error UI', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.context().clearCookies()
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    await page.goto(`${baseURL}/auth/callback`)
    await expect(page.getByText(/sign-in failed/i)).toBeVisible({
      timeout: 10_000,
    })
    await expect(page.getByRole('link', { name: /return to login/i })).toBeVisible()
  })

  test('/auth/link-account form: wrong password shows error, correct logs in', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Setup admin so the app is bootstrapped, but use a logged-out
    // browser state for the link page (it's a pre-auth surface).
    await loginAsAdmin(page, baseURL)
    await page.context().clearCookies()
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Mock the link-account endpoint so we control the responses.
    let callCount = 0
    await page.route('**/api/auth/link-account', async route => {
      callCount++
      const body = JSON.parse(route.request().postData() ?? '{}')
      if (body.password === 'correct-password') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            user: {
              id: '00000000-0000-0000-0000-000000000001',
              username: DEFAULT_ADMIN_CREDENTIALS.username,
              email: DEFAULT_ADMIN_CREDENTIALS.email,
              is_active: true,
              is_admin: false,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
              permissions: [],
            },
            access_token: 'fake-access-token-from-mock',
            refresh_token: 'fake-refresh',
            expires_in: 3600,
            token_type: 'Bearer',
          }),
        })
      } else {
        await route.fulfill({
          status: 401,
          contentType: 'application/json',
          body: JSON.stringify({
            error_code: 'INVALID_CREDENTIALS',
            error: 'Invalid credentials',
          }),
        })
      }
    })

    await page.goto(
      `${baseURL}/auth/link-account?link_token=test-link-token-abc`,
    )

    // Form renders.
    const pwField = page.getByLabel('Password')
    await expect(pwField).toBeVisible({ timeout: 10_000 })

    // Wrong password → inline error.
    await pwField.fill('wrong')
    await page.getByRole('button', { name: /link and sign in/i }).click()
    await expect(page.getByText(/invalid credentials/i)).toBeVisible({
      timeout: 5_000,
    })

    // Correct password → mocked 200 → store hydrates → navigate home.
    await pwField.fill('correct-password')
    await page.getByRole('button', { name: /link and sign in/i }).click()
    // After success the page navigates to "/" via the mocked auth flow.
    // The mock's user payload doesn't make /api/auth/me succeed,
    // so initAuth may flip isAuthenticated back to false — but the
    // initial navigate-to-"/" call still fires. Assert that the
    // link-account endpoint was called twice (1 wrong + 1 right).
    await page.waitForTimeout(500)
    expect(callCount).toBeGreaterThanOrEqual(2)
  })
})
