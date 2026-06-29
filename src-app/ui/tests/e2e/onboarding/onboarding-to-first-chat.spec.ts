import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToGroupViaAPI,
} from '../../common/provider-helpers'
import {
  sendChatMessage,
  waitForNewChatPageLoad,
} from '../chat/helpers/chat-helpers'

/**
 * E2E (real-LLM) — the WHOLE first-run journey in one spec:
 * fresh user → onboarding wizard → land in chat → send a first message →
 * see a real assistant response.
 *
 * Audit gap (all-3f998eae538d): onboarding-wizard.spec.ts covers the wizard and
 * the chat specs start from an already-configured state, but nothing wired the
 * two together — a brand-new user stepping out of onboarding and actually
 * getting a model reply. This deployment-configures a real Anthropic Haiku model
 * (assigned to the default group the fresh user auto-joins), runs the skippable
 * guide to completion, then sends a message from the chat the user lands on and
 * asserts a non-empty assistant turn renders. Only the real LLM is external;
 * everything else is the production path. Soft-skips without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC = (process.env.ANTHROPIC_API_KEY ?? '').length > 0

test.describe('Onboarding → first chat message (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('a fresh user onboards and gets a reply to their first message', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // Creates + completes the admin so it isn't itself trapped in onboarding.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Deployment-wide: a REAL Anthropic provider + Haiku model, assigned to the
    // default group that every new user auto-joins — so the user who walks out
    // of onboarding lands on a functional, model-backed chat.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )
    const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
      headers: { Authorization: `Bearer ${adminToken}` },
    })
    const { groups } = await groupsRes.json()
    const defaultGroup =
      groups.find((g: any) => g.is_default) ??
      groups.find((g: any) => g.name === 'Users')
    expect(defaultGroup, 'a default group must exist').toBeTruthy()
    await assignProviderToGroupViaAPI(apiURL, adminToken, defaultGroup.id, [
      providerId,
    ])

    // Brand-new user → bounced into the wizard on first login.
    const username = `onb2chat_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Click straight through the skippable getting-started guide.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(
      byTestId(page, 'onboarding-step-api-keys'),
    ).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(
      byTestId(page, 'onboarding-step-memory-setup'),
    ).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-finish')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()

    // Landed on a real chat (AuthGuard released; composer present).
    await expect(page).toHaveURL(/\/chat/, { timeout: 15000 })
    await waitForNewChatPageLoad(page)

    // The first message gets a real reply — the journey's payoff.
    await sendChatMessage(page, 'In one short sentence, say hello.')

    const assistant = page.locator('[data-role="assistant"]').last()
    await expect(assistant).toBeVisible({ timeout: 60000 })
    expect(((await assistant.textContent()) ?? '').trim().length).toBeGreaterThan(
      0,
    )
  })
})
