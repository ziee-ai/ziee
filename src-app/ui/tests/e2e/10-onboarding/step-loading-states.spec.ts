import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * The data-driven onboarding steps (ApiKeysStep / McpServersStep /
 * MemorySetupStep) render a centered <Spin/> while their initial fetch is in
 * flight, before the real content. This pins the loading branch by HOLDING the
 * AI-Providers fetch open: the Spin must be visible, and once the response is
 * released the step content (the "AI Providers" heading) replaces it.
 */

async function freshUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
  ])
  return username
}

test.describe('Onboarding step loading states', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('AI Providers step shows a Spin until its fetch resolves', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await freshUser(apiURL, 'loadspin')

    // Gate the user-providers fetch so we can observe the loading branch.
    let release!: () => void
    const gate = new Promise<void>((r) => {
      release = r
    })
    await page.route(/\/api\/user-llm-providers(\?|$)/, async (route) => {
      await gate
      await route.continue()
    })

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()

    // The fetch is held open → the loading Spin renders (no content heading yet).
    await expect(page.locator('.ant-spin')).toBeVisible()

    // Release the fetch → the loading branch is replaced by the real step.
    release()
    await expect(
      page.getByRole('heading', { name: 'AI Providers' }),
    ).toBeVisible({ timeout: 15000 })
  })
})
