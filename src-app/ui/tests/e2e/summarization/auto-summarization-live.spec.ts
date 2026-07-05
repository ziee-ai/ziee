import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../chat/helpers/chat-helpers'

/**
 * E2E — AUTOMATIC summarization fires during a real chat.
 *
 * The existing summarization specs either seed a `conversation_summaries`
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
          // The backend clamps these to valid ranges: trigger ∈ 500..=1e6 and
          // keep_recent >= 100 (and < trigger). Use the minimum trigger so a
          // few substantive turns cross it.
          summarize_after_tokens: 500,
          summarizer_keep_recent_tokens: 100,
        },
      },
    )
    expect(
      settingsResp.ok(),
      `settings PUT should succeed: ${settingsResp.status()} ${await settingsResp.text()}`,
    ).toBeTruthy()

    // Drive a real conversation: enough detailed turns to push cumulative
    // tokens well past the 500 trigger so the background summarizer fires.
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    await sendChatMessage(
      page,
      'Plan a detailed multi-day itinerary for a trip to Tokyo, Japan, with specific neighborhoods and attractions for each day.',
    )
    await sendChatMessage(
      page,
      'Add Kyoto and Osaka as well, with several food recommendations and a signature dish for each city.',
    )
    await sendChatMessage(
      page,
      'Now break down the budget considerations for the whole trip: flights, lodging, food, and local transport.',
    )
    await sendChatMessage(
      page,
      'Finally, suggest a packing list and a few useful Japanese phrases for the trip.',
    )

    // The background summarizer runs AFTER a reply and is itself an async LLM
    // call, so the boundary row may not exist yet at the first reload. The
    // summary read-model reloads on each conversation open, so poll by
    // reloading until the boundary divider renders (it only appears when a
    // summary row exists — auto-created here, never seeded).
    await expect(async () => {
      await page.reload()
      await expect(byTestId(page, 'summ-boundary-toggle').first()).toBeVisible({
        timeout: 8_000,
      })
    }).toPass({ timeout: 120_000, intervals: [4_000] })
  })
})
