import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — session survives a HARD page reload.
 *
 * Audit gap: Auth.store.ts persists the token via the zustand persist
 * middleware and AuthGuard re-bootstraps from it, but no test verified
 * that a full browser reload keeps the user authenticated (rather than
 * bouncing to /auth). This logs in, deep-navigates, hard-reloads, and
 * asserts the authenticated shell is still mounted and the token persisted.
 */

test.describe('Auth — session persistence across reload', () => {
  test('a hard reload keeps the user authenticated', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'profile-account-card')).toBeVisible({
      timeout: 30000,
    })

    // Hard reload — drops all in-memory React/zustand state.
    await page.reload({ waitUntil: 'domcontentloaded' })

    // The persisted token is still present...
    const hasToken = await page.evaluate(() => {
      const raw = localStorage.getItem('auth-storage')
      if (!raw) return false
      try {
        return Boolean(JSON.parse(raw).state?.token)
      } catch {
        return false
      }
    })
    expect(hasToken).toBe(true)

    // ...and the user is NOT bounced to the login page; the authed
    // profile page re-mounts from the persisted session.
    await expect(page).not.toHaveURL(/\/auth/)
    await expect(byTestId(page, 'profile-account-card')).toBeVisible({
      timeout: 30000,
    })
  })
})
