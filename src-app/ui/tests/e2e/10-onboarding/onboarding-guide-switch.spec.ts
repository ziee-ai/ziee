import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E for the onboarding guide-selector sidebar (OnboardingPage.tsx
 * `handleSelectGuide`, lines ~103-106 + the left-pane `guides.map`).
 *
 * Clicking a guide in the left sidebar calls `handleSelectGuide`, which
 * sets the active guide AND clears manual step navigation
 * (`setManualStep(null)`) — so the viewer jumps back to the guide's
 * computed resume point (the first not-yet-completed step) regardless of
 * where the user had manually paged to.
 *
 * Note on scope: the shipped app registers exactly ONE onboarding guide
 * ("Getting Started"), so a true *inter-guide* switch (guide A -> guide B)
 * is not reachable in production — the sidebar lists a single entry. This
 * spec therefore exercises the reachable behavior of the same selector
 * control: re-selecting the guide mid-flow discards a manual "Back" and
 * snaps to the resume step. (If a second guide is ever registered, the
 * same `getByText(<guide title>)` click drives a genuine A->B switch.)
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

test.describe('Onboarding — guide selector', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    // Creates + onboards the admin so it isn't trapped on /onboarding.
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('re-selecting the guide mid-flow snaps back to the resume step', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'guideswitch')

    await loginExpectingOnboarding(page, baseURL, username, 'password123')
    await expect(page).toHaveURL(/\/onboarding/)

    // The sidebar lists the registered guide as a clickable card (its
    // description is unique to the sidebar entry, so it disambiguates from
    // the right-pane "Getting Started" heading).
    const guideCard = page.getByText(
      'Set up your AI providers and MCP servers to get started.',
    )
    await expect(guideCard).toBeVisible()

    // Step 0 (Welcome) is showing.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()

    // Advance to step 1 (AI Providers). This completes "welcome", so the
    // guide's resume point becomes step 1.
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()

    // Manually page Back to the (now-completed) Welcome step.
    await page.getByRole('button', { name: 'Back' }).click()
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()

    // Click the guide in the sidebar — handleSelectGuide clears the manual
    // step and recomputes the resume point, snapping forward to the first
    // incomplete step (AI Providers), NOT staying on Welcome.
    await guideCard.click()
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()
    await expect(page.getByRole('heading', { name: /Welcome/ })).toHaveCount(0)
  })
})
