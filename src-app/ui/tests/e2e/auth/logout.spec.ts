import { test, expect } from '../../fixtures/test-context'
import {
  completeOnboarding,
  createTestUser,
  getAdminToken,
  loginAsAdmin,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — logout via the UserProfileWidget dropdown.
 *
 * `UserProfileWidget` renders a Dropdown (trigger `[data-testid=
 * "user-profile-widget"]`) whose "Logout" item calls `Stores.Auth.logoutUser()`.
 * After logout the AuthGuard renders the AuthPage. This drives the real
 * click-path (no API shortcut) and asserts the user lands back on the login form.
 */

test.describe('Authentication — logout', () => {
  test('logging out via the profile dropdown returns to the login form', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    // Open the profile dropdown in the sidebar footer.
    const widget = byTestId(page, 'user-profile-widget')
    await expect(widget).toBeVisible({ timeout: 30000 })
    await widget.click()

    // Click the "Logout" menu item.
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()

    // The AuthPage login form replaces the app shell.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    // The authenticated sidebar widget is gone.
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)
  })
  // The stored token must be gone, not just the rendered UI: a reloaded tab
  // rehydrates `token` from this key, so a surviving value would resurrect the
  // session.
  //
  // HONEST SCOPE: this holds on the base branch too (the pre-existing wipe
  // already nulled `token`). It is a guard against the teardown refactor
  // REGRESSING that property, not evidence of the new revocation — the
  // discriminating specs are the backstop test below and the cross-tab test in
  // sync/session-sync.spec.ts.
  test('logout clears the persisted token', async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({
      timeout: 15000,
    })

    // The teardown reloads the document, so a bare page.evaluate here races the
    // navigation ("Execution context was destroyed"). Retry until it settles.
    await expect(async () => {
      const persisted = await page.evaluate(() =>
        localStorage.getItem('auth-storage'),
      )
      expect(persisted, 'auth-storage should still exist').toBeTruthy()
      expect(JSON.parse(persisted as string)?.state?.token).toBeNull()
    }).toPass({ timeout: 15000 })
  })

  // The SERVER-SIDE backstop, isolated from the cross-tab sync signal.
  //
  // Device B is its OWN browser context — independent localStorage + cookies —
  // and logs in separately, so it holds its own copy of the access token. That
  // is what makes this a real test: it must still be dead after device A logs
  // out, even though nothing clears B's stored token and B never receives the
  // Session signal (its SSE stream is blocked). The only thing that can stop it
  // is the server rejecting the revoked token.
  //
  // NOTE: a SAME-context second tab would prove nothing here — tab A's logout
  // nulls the shared `auth-storage` token, so tab B would land on the login form
  // via persist rehydration alone, even with the server-side revocation removed.
  test('a device with no sync stream is still dead after another device logs out', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const adminToken = await getAdminToken(apiURL)
    const username = `backstop_${Date.now().toString(36)}`
    const password = 'backstopPass123'
    await createTestUser(apiURL, adminToken, username, `${username}@example.com`, password, [])

    // Device B needs onboarding done, or AuthGuard lands it on the wizard.
    const bLogin = await fetch(`${apiURL}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    })
    await completeOnboarding(baseURL, (await bLogin.json()).access_token)

    // Device B: separate context ⇒ its own localStorage; its own login.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      // Block the SSE subscribe so B never learns of the logout out-of-band.
      await pageB.route('**/api/sync/subscribe*', route => route.abort())

      // Log in through the REAL form, NOT the `login` helper: that helper seeds
      // the token via addInitScript, which re-runs on EVERY navigation — so the
      // teardown's reload would re-inject the revoked token and loop forever.
      // Here the app itself stores the token, exactly as a real browser does.
      await pageB.goto(`${baseURL}/`)
      await byTestId(pageB, 'auth-login-username').fill(username)
      await byTestId(pageB, 'auth-login-password').fill(password)
      await byTestId(pageB, 'auth-login-submit').click()
      await expect(byTestId(pageB, 'user-profile-widget')).toBeVisible({
        timeout: 30000,
      })

      // Capture B's OWN token, then log B's user out from a different client
      // entirely (device A's request, B's credentials) — B's stored token is
      // untouched.
      const tokenB = await pageB.evaluate(
        () => JSON.parse(localStorage.getItem('auth-storage') || '{}')?.state?.token,
      )
      expect(tokenB, "device B should hold its own token").toBeTruthy()

      const logoutRes = await page.request.post(`${baseURL}/api/auth/logout`, {
        headers: { Authorization: `Bearer ${tokenB}` },
      })
      expect(logoutRes.status()).toBe(204)

      // B still HAS the token in storage — prove the SERVER refuses it.
      const stillStored = await pageB.evaluate(
        () => JSON.parse(localStorage.getItem('auth-storage') || '{}')?.state?.token,
      )
      expect(stillStored, "B's token must still be in storage — the server is what stops it").toBe(
        tokenB,
      )
      const status = await pageB.evaluate(async t => {
        const r = await fetch('/api/conversations', {
          headers: { Authorization: `Bearer ${t}` },
        })
        return r.status
      }, tokenB)
      expect(status, 'the revoked token must be refused by the server').toBe(401)

      // And the UI tears down on B's next real interaction.
      await pageB.goto(`${baseURL}/settings`)
      await pageB.waitForLoadState('load')
      await expect(byTestId(pageB, 'auth-login-username')).toBeVisible({
        timeout: 30000,
      })
    } finally {
      await ctxB.close()
    }
  })
})
