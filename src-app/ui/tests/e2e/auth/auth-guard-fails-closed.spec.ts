import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'

/**
 * E2E — AuthGuard FAILS CLOSED on an invalid / tampered token.
 *
 * RouterComponent.tsx:157-165 + AuthGuard.tsx: a guard is a security
 * control, so anything other than a positively-verified session must seal
 * the protected routes behind the login wall — never render protected
 * content "fail-open".
 *
 * The existing redirect specs cover the PLAIN-unauthenticated case
 * (`about-unauthenticated-redirect.spec.ts` — no token at all) and the
 * onboarding redirect (`guarded-route-redirect.spec.ts`). The genuinely
 * uncovered angle is a token that is PRESENT but BOGUS: a stale/forged
 * `auth-storage` token. `initAuth()` verifies it via `/auth/me`, the server
 * returns 401, the store wipes the token + holds `isAuthenticated=false`,
 * and AuthGuard renders `<AuthPage />`. A regression that trusted a
 * client-side token without server verification (fail-open) would leak the
 * protected page — this asserts it does not.
 *
 * No mocks: the real `/auth/me` 401, the real store wipe, and the real
 * AuthGuard all run.
 */

/** Seed a bogus token into the persisted auth store before any app code runs. */
async function seedBogusToken(
  page: import('@playwright/test').Page,
  token: string,
) {
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
}

// A guarded deep-link whose content must never appear without a real session.
const GUARDED_ROUTE = '/settings/profile'

test.describe('Auth — AuthGuard fails closed', () => {
  test('a tampered token is bounced to the login wall, not the protected page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // A structurally-bogus token: present in storage, but not a token the
    // server will accept → `/auth/me` 401.
    await seedBogusToken(page, 'tampered.not-a-real.jwt')
    await page.goto(`${baseURL}${GUARDED_ROUTE}`)

    // FAIL CLOSED: the login wall is shown, protected content is not.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    // The profile page's own controls must NOT have rendered.
    await expect(byTestId(page, 'profile-display-name-input')).toHaveCount(0)
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)

    // The store rejected + wiped the bad token (it was not trusted): the
    // persisted token is cleared after the 401.
    await expect
      .poll(
        async () =>
          await page.evaluate(() => {
            try {
              const raw = localStorage.getItem('auth-storage')
              if (!raw) return null
              return JSON.parse(raw)?.state?.token ?? null
            } catch {
              return 'parse-error'
            }
          }),
        { timeout: 15000 },
      )
      .toBeNull()
  })

  test('the catch-all fallback route also seals a tampered token to login', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // An unknown deep-link hits the `path="*"` route, which is wrapped in the
    // SAME guard (RouterComponent.tsx:200) — it must fail closed too.
    await seedBogusToken(page, 'another.bogus.token')
    await page.goto(`${baseURL}/this-route-does-not-exist-${Date.now()}`)

    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)
  })
})
