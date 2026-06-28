import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from '../09-chat/fixtures/mock-tool-result'

/**
 * E2E — the UI counterpart to `per-conversation-toggle.spec.ts`'s
 * "GET /summary returns null" API assertion.
 *
 * `SummaryBoundaryMarker` (summarization/chat-extension) is registered in the
 * `message_footer` slot, so EVERY rendered `ChatMessage` mounts an instance.
 * Each instance bails out (`SummaryBoundaryMarker.tsx:37`,
 * `if (!current?.summary) return null`) when the conversation has no summary.
 * The API-null case is covered; this asserts the rendered-UI consequence: with
 * real message bubbles on screen (so the marker code path actually runs) and a
 * null summary, NO condensed-summary boundary marker appears in the thread.
 */

test.describe('Summarization — boundary marker hidden when summary is null', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('no condensed-summary boundary marker renders when the summary is null', async ({
    page,
    testInfra,
  }) => {
    // Seed a real user + assistant turn so ChatMessage bubbles mount and each
    // one's `message_footer` SummaryBoundaryMarker instance executes.
    const { assistantMessageId } = await seedAssistantWithToolResult(
      page,
      testInfra.baseURL,
      {
        resourceLinks: [],
        text: 'Here is the answer to your question.',
      },
    )

    // Positive control: the seeded assistant bubble is on screen, so the
    // marker code path genuinely ran (and bailed) — this is not an
    // empty-thread false pass.
    await expect(
      page
        .locator(`[data-testid="chat-message"][data-message-id="${assistantMessageId}"]`)
        .first(),
    ).toBeVisible({ timeout: 15000 })

    // The conversation has no summary row → `current.summary` is null →
    // SummaryBoundaryMarker returns null for every message. Neither the
    // expandable divider control nor its label text may appear.
    await expect(
      page.getByRole('button', { name: /condensed-conversation summary/i }),
    ).toHaveCount(0)
    await expect(
      page.getByText(/messages condensed into a summary/i),
    ).toHaveCount(0)
  })
})
