import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the headline silent-refresh behavior: an ACTIVE session survives
 * access-token expiry without bouncing the user to the login page.
 *
 * The backend runs with a REAL 8-second access-token TTL (debug-only
 * `jwt.access_token_expiry_seconds` seam, wired via the
 * `jwtAccessExpirySeconds` test option). The client's proactive refresh
 * fires at 75% of lifetime (~6s), rotating the token via the httpOnly
 * `ziee_refresh` cookie — so waiting past the original expiry with the
 * app open must (1) keep the user on the app, (2) leave a DIFFERENT
 * access token in localStorage (rotation proof), and (3) keep API-backed
 * pages working. Reloading after expiry exercises the reactive path
 * (initAuth's /me 401 → on-401 interceptor → cookie refresh → retry).
 */

test.describe('Auth — session silent refresh past token expiry', () => {
  // 8s TTL: long enough for a stable login, short enough to expire
  // inside the test. Proactive refresh fires at ~6s.
  test.use({ jwtAccessExpirySeconds: 8 })

  /** The persisted access token (localStorage `auth-storage`). */
  const readToken = () =>
    JSON.parse(localStorage.getItem('auth-storage') ?? '{}').state?.token ??
    null

  test('an active session survives past access-token expiry (proactive refresh)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const tokenAtLogin = await page.evaluate(readToken)
    expect(tokenAtLogin).toBeTruthy()

    // Stay on the app well past the 8s expiry of the login token. The
    // proactive timer (75% ≈ 6s) must rotate it under us.
    await page.waitForTimeout(12_000)

    // (1) Not bounced to the login page.
    await expect(page).not.toHaveURL(/\/auth/)

    // (2) The access token CHANGED — the silent refresh actually ran
    //     (not merely "nothing noticed the expiry yet").
    const tokenAfterExpiry = await page.evaluate(readToken)
    expect(tokenAfterExpiry).toBeTruthy()
    expect(tokenAfterExpiry).not.toBe(tokenAtLogin)

    // (3) An API-backed interaction works with the refreshed token: the
    //     profile page loads its card (GET /auth/me + profile data).
    await page.goto(`${baseURL}/settings/profile`, {
      waitUntil: 'domcontentloaded',
    })
    await expect(page.getByTestId('profile-account-card')).toBeVisible({
      timeout: 30_000,
    })
    await expect(page).not.toHaveURL(/\/auth/)
  })

  test('a reload AFTER expiry recovers the session via the refresh cookie', async ({
    page,
    testInfra,
  }) => {
    // Parking past the 8s TTL + the 5s JWT validation leeway before the
    // token is actually rejected makes this longer than the default.
    test.setTimeout(90_000)
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const tokenAtLogin = await page.evaluate(readToken)
    expect(tokenAtLogin).toBeTruthy()

    // Park the browser OFF the app so no client-side refresh machinery
    // runs while the access token expires for real. The httpOnly
    // ziee_refresh cookie survives in the browser jar. Wait past the 8s
    // TTL + the 5s validation leeway so the token is genuinely rejected
    // on reload (exercising the reactive on-401 → refresh → retry path).
    await page.goto('about:blank')
    await page.waitForTimeout(16_000)

    // Cold-load the app with an EXPIRED access token in localStorage:
    // initAuth's /me 401s → the on-401 interceptor refreshes via the
    // cookie → the retry succeeds → the user lands authenticated.
    await page.goto(`${baseURL}/settings/profile`, {
      waitUntil: 'domcontentloaded',
    })
    await expect(page.getByTestId('profile-account-card')).toBeVisible({
      timeout: 30_000,
    })
    await expect(page).not.toHaveURL(/\/auth/)

    // Rotation proof: the recovered session runs on a fresh token.
    const tokenAfterReload = await page.evaluate(readToken)
    expect(tokenAfterReload).toBeTruthy()
    expect(tokenAfterReload).not.toBe(tokenAtLogin)
  })

  test('the realtime-sync stream re-establishes after a silent refresh', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const tokenAtLogin = await page.evaluate(readToken)

    // The SSE stream is deliberately torn down at the token's exp
    // (sync/handlers.rs bounds it by the JWT deadline). With the silent
    // refresh keeping a fresh token in storage, the SyncClient's
    // reconnect must land a NEW successful /api/sync/subscribe after
    // the original token's expiry.
    const resubscribed = page.waitForResponse(
      res =>
        res.url().includes('/api/sync/subscribe') && res.status() === 200,
      { timeout: 30_000 },
    )
    await page.waitForTimeout(9_000) // past the 8s exp of the login token
    await resubscribed

    // And the session itself rolled rather than died.
    await expect(page).not.toHaveURL(/\/auth/)
    const tokenNow = await page.evaluate(readToken)
    expect(tokenNow).not.toBe(tokenAtLogin)
  })

  test('logout kills the silent refresh (no token resurrection, no leftover refresh)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Watch for ANY /api/auth/refresh call from logout onward — after a
    // real sign-out there must be none (a leftover proactive timer or
    // watchdog would surface here). Registered BEFORE the logout click.
    let sawRefresh = false
    page.on('request', req => {
      if (req.url().includes('/api/auth/refresh')) sawRefresh = true
    })

    // Log out via the REAL sidebar widget control (drives
    // Stores.Auth.logoutUser — revokes refresh tokens server-side, clears
    // the httpOnly cookie, bumps the session epoch, stops the timers).
    // No navigation, so any leftover refresh in THIS page context is
    // observable below.
    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()

    // AuthGuard renders the login wall inline once isAuthenticated flips.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({
      timeout: 30_000,
    })

    // Wait past a proactive-refresh cycle + a watchdog tick: no refresh,
    // no token resurrection.
    await page.waitForTimeout(8_000)
    expect(sawRefresh, 'no silent refresh may fire after logout').toBe(false)
    const token = await page.evaluate(readToken)
    expect(token).toBeNull()
  })
})
