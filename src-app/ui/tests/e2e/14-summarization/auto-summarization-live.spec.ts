import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../09-chat/helpers/chat-helpers'

/**
 * E2E — AUTOMATIC summarization fires during a real chat.
 *
 * The existing 14-summarization specs either seed a `conversation_summaries`
 * row directly (`in-thread-marker`) or drive admin settings — none exercise the
 * live `after_llm_call` → `refresh_summary` path that condenses old turns once a
 * conversation crosses the token threshold. This closes that gap end-to-end:
 *
 *   1. Real Anthropic provider + Haiku model (admin-granted).
 *   2. Lower the deployment-wide trigger to a tiny token budget via
 *      PUT /api/summarization/settings so a few short turns cross it.
 *   3. Send several real messages through the chat UI.
 *   4. Assert the in-thread boundary divider ("… condensed into a summary")
 *      appears — proving the background summarizer ran and persisted, with NO
 *      seeding.
 *
 * Real-LLM tier: soft-skipped without ANTHROPIC_API_KEY (mirrors
 * run-a-workflow / chat-stream-sync). Run with --workers=1.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

test.describe('Summarization — automatic trigger during a real chat', () => {
  test.skip(
    ANTHROPIC_KEY.length === 0,
    'ANTHROPIC_API_KEY not set — real-LLM auto-summarization E2E skipped',
  )

  test('crossing the token threshold condenses old turns into a summary', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
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

    // Enable summarization deployment-wide and set a tiny trigger so a couple
    // of short turns exceed it (keep-recent must stay < trigger).
    const settingsResp = await page.request.put(
      `${apiURL}/api/summarization/settings`,
      {
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${adminToken}`,
        },
        data: {
          enabled: true,
          summarize_after_tokens: 40,
          summarizer_keep_recent_tokens: 10,
        },
      },
    )
    expect(settingsResp.ok()).toBeTruthy()

    // Drive a real conversation: enough turns to push cumulative tokens past 40.
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Let us plan a detailed multi-day itinerary for a trip to Tokyo, Japan.',
    )
    await sendChatMessage(
      page,
      'Add Kyoto and Osaka as well, with food recommendations for each city.',
    )
    await sendChatMessage(
      page,
      'Now summarize the budget considerations for the whole trip in brief.',
    )

    // The background summarizer runs after a reply; the pill reloads the read
    // model on the next messages.size change. Reload to force a clean read,
    // then poll for the boundary divider that only renders when a summary row
    // exists (auto-created, never seeded here).
    await page.reload()
    await expect(
      page.getByText(/condensed into a summary/i).first(),
    ).toBeVisible({ timeout: 60_000 })
  })
})
