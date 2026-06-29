import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding mid-flow navigation (`OnboardingPage.tsx:103-106` guide
 * selection + the Back/Next manual-step navigation at 145-170/footer).
 *
 * The wizard spec only walks forward. This drives BACKWARD navigation
 * mid-onboarding (Back returns to the prior step) and re-selecting the active
 * guide in the sidebar (`handleSelectGuide` → resets the manual step), keeping
 * the user on Getting Started.
 */

async function freshUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
  ])
  return { username }
}

test.describe('Onboarding — mid-flow navigation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('Back returns to the prior step and re-selecting the guide stays on it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'nav')
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Forward: Welcome → AI Providers → MCP Servers.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()

    // Backward (mid-onboarding): Back → AI Providers → Welcome.
    await byTestId(page, 'onboarding-page-back-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await byTestId(page, 'onboarding-page-back-button').click()
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    // Back is disabled on the first step.
    await expect(byTestId(page, 'onboarding-page-back-button')).toBeDisabled()

    // Re-select the "Getting Started" guide entry in the sidebar
    // (handleSelectGuide) → stays on the same guide.
    await byTestId(page, 'onboarding-guide-card-getting-started').click()
    await expect(byTestId(page, 'onboarding-guide-title')).toContainText('Getting Started')
  })
})
