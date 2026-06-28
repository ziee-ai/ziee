import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — the onboarding steps render a loading spinner while their data
 * loads (ApiKeysStep.tsx `if (loading) return <Spin/>`).
 *
 * Audit gap: the steps' loading/skeleton branches were never asserted. This
 * delays the providers GET (the external boundary the AI-Providers step
 * awaits) and asserts the spinner renders on that step before data arrives.
 */

test.describe('Onboarding — loading state', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('AI Providers step shows a spinner while providers load', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `skel_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )

    // Delay the providers GET so the step's loading branch is observable.
    await page.route('**/api/user-llm-providers', async route => {
      if (route.request().method() !== 'GET') return route.fallback()
      await new Promise(r => setTimeout(r, 4000))
      await route.fallback()
    })

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(
      page.getByRole('heading', { name: 'AI Providers' }),
    ).toBeVisible()

    // The providers GET is still in flight → the step renders its spinner.
    await expect(page.locator('.ant-spin').first()).toBeVisible({
      timeout: 4000,
    })
  })
})
