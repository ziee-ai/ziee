import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  createConversationWithModel,
  waitForAssistantResponse,
} from '../09-chat/helpers/chat-helpers'

/**
 * Full first-run journey smoke: real /setup form → onboarding completed → land
 * on the app shell → configure a provider/model → send the FIRST chat message
 * and get a real assistant response. The individual legs are covered piecemeal
 * (onboarding-wizard.spec, chat specs); this chains them on a brand-new
 * instance so a regression that breaks the path between "just set up" and
 * "first message answered" is caught.
 *
 * Real-LLM tier: needs a model to answer. Gated on ANTHROPIC_API_KEY.
 */
const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

test.describe('First-run journey — setup → onboarding → first chat', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('a freshly set-up admin can send a first chat message and get a reply', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // Drives the REAL first-run /setup form on a fresh backend, then completes
    // the onboarding guide and lands on the authenticated app shell (the
    // sidebar "New Chat" menuitem is the readiness signal inside this helper).
    await loginAsAdmin(page, baseURL)

    // Configure a real provider + a fast model so the first message can be
    // answered.
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // First chat message → real streamed assistant reply.
    await createConversationWithModel(
      page,
      baseURL,
      'Claude Haiku 4.5',
      'Reply with the single word: READY',
    )
    await waitForAssistantResponse(page)

    // An assistant message rendered for the first conversation.
    await expect(page.locator('[data-role="assistant"]').first()).toBeVisible()
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/)
  })
})
