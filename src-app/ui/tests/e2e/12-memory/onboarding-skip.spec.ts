import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding Memory step: SKIP path.
 *
 * Plan §9 Phase 1: "admin walks through onboarding, picks Skip on the
 * Memory step; assert memory stays disabled, Memory admin page is
 * reachable, dropdown is empty."
 */

test.describe('Memory — onboarding skip', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('admin skips memory step; memory stays disabled', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `skip_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::read',
        'memory::write',
        'memory::admin::read',
        'memory::admin::manage',
      ],
    )

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Step through the wizard via the Next button until we reach
    // Memory, then Skip.
    // (Selectors are scaffolded; actual wizard markup is in
    // OnboardingPage.tsx and may evolve.)
    await page.getByRole('button', { name: /Next/ }).click() // Welcome
    await page.getByRole('button', { name: /Next/ }).click() // API Keys
    await page.getByRole('button', { name: /Next/ }).click() // MCP

    // Memory step: leave switch off, click Next.
    await expect(page.getByRole('heading', { name: /Persistent Memory/ })).toBeVisible()
    await page.getByRole('button', { name: /Next/ }).click()

    // Finish step. OnboardingPage.tsx:231 labels the last-step button
    // "Start Chatting" (not "Finish" / "Done").
    await page.getByRole('button', { name: /Start Chatting/ }).click()

    // Admin settings page reachable; memory still disabled.
    const userToken = await getCurrentUserToken(page)
    const adminRes = await page.request.get(
      `${apiURL}/api/memory/admin-settings`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    expect(adminRes.status()).toBe(200)
    const settings = await adminRes.json()
    expect(settings.enabled).toBe(false)
  })
})
