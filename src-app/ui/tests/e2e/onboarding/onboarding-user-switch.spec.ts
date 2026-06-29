import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  clearAuthState,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * Onboarding store user-switch isolation (gap 72d8).
 *
 * Onboarding.store.ts subscribes to the auth user id and, on a switch, clears
 * `completedGuideIds`/`completedStepIds` and reloads (with a monotonic
 * `loadToken` so a superseded in-flight load can't overwrite the new user's
 * progress). This guards the observable contract: a user who switches from an
 * account with partial onboarding progress to a FRESH account must see the
 * wizard at the first step — never the prior user's advanced position.
 */

const GUIDE = 'getting-started'

async function makeUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
  ])
  return username
}

test.describe('Onboarding store — user switch isolation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('fresh user after a partially-onboarded user sees the wizard at step 1', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // User A: advance past the welcome step (partial progress).
    const userA = await makeUser(apiURL, 'switch_a')
    const aToken = await getAdminToken(apiURL, { username: userA, password: 'password123' })
    const res = await fetch(`${apiURL}/api/onboarding/${GUIDE}/steps/welcome/complete`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${aToken}` },
    })
    expect(res.ok).toBeTruthy()

    await loginExpectingOnboarding(page, baseURL, userA, 'password123')
    // A resumes PAST welcome — at AI Providers (proves A has stored progress).
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()

    // Switch to a brand-new user B with NO progress.
    await clearAuthState(page)
    const userB = await makeUser(apiURL, 'switch_b')
    await loginExpectingOnboarding(page, baseURL, userB, 'password123')

    // B must start at Welcome — A's completed-step progress must NOT leak.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeHidden()
  })
})
