import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding ApiKeysStep "No AI providers enabled" empty state
 * (audit 4db6743cef50).
 *
 * ApiKeysStep.tsx:55-69 renders an empty state ("No AI providers are
 * currently enabled. An administrator can add providers in the Admin
 * settings.") when the onboarding user can see zero enabled, non-local
 * LLM providers. Every existing onboarding spec (e.g. wizard-api-key-save)
 * first SEEDS a provider visible to the user, so the providers-empty branch
 * was never exercised. Each test runs against its own fresh per-test
 * database, so a brand-new user with no provider assignment deterministically
 * hits the empty state — no mocks.
 */

test.describe('Onboarding — AI Providers step empty state', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('shows the "No AI providers enabled" empty state when none are configured', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    // A brand-new onboarding user — deliberately NO provider created/assigned,
    // so `getUserLlmProviders` returns an empty list for them.
    const username = `noprov_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(
      byTestId(page, 'onboarding-step-api-keys'),
    ).toBeVisible()

    // The empty-state copy from ApiKeysStep.tsx renders…
    await expect(byTestId(page, 'onboarding-apikeys-empty')).toBeVisible({
      timeout: 10_000,
    })
    await expect(byTestId(page, 'onboarding-apikeys-empty')).toContainText(
      /No AI providers are currently enabled/i,
    )
    await expect(byTestId(page, 'onboarding-apikeys-empty')).toContainText(
      /administrator can add/i,
    )

    // …and the provider picker (the non-empty branch's API-key form) is absent.
    await expect(byTestId(page, 'onboarding-apikeys-password-input')).toHaveCount(0)

    // The wizard can still advance past the (empty) providers step.
    await expect(byTestId(page, 'onboarding-page-next-button')).toBeEnabled()
  })
})
