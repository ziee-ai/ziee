import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — the wizard drives the onboarding PROGRESS API (Onboarding.store.ts
 * completeStep/completeGuide) through the UI.
 *
 * Audit gap: existing specs assert the end-state (AuthGuard releases) but
 * never pin that the step-complete + guide-complete REST endpoints actually
 * fire from the wizard buttons. This steps through the wizard and asserts
 * BOTH POST /api/onboarding/{guide}/steps/{step}/complete and
 * POST /api/onboarding/{guide}/complete are issued.
 */

test.describe('Onboarding — progress API via UI', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('Next issues step-complete and Start Chatting issues guide-complete', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `progapi_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()

    // Clicking "Next" marks the current step complete via the step endpoint.
    const stepComplete = page.waitForResponse(
      r =>
        /\/api\/onboarding\/.+\/steps\/.+\/complete$/.test(r.url()) &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await byTestId(page, 'onboarding-page-next-button').click()
    expect((await stepComplete).status()).toBeLessThan(400)

    // Walk the rest of the wizard to the finish step.
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(
      byTestId(page, 'onboarding-step-memory-setup'),
    ).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-finish')).toBeVisible()

    // "Start Chatting" completes the guide via the guide-complete endpoint.
    const guideComplete = page.waitForResponse(
      r =>
        /\/api\/onboarding\/[^/]+\/complete$/.test(r.url()) &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await byTestId(page, 'onboarding-page-next-button').click()
    expect((await guideComplete).status()).toBeLessThan(400)
  })
})
