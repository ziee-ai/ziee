import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — In-thread summary boundary marker.
 *
 * Seeds a conversation with synthetic messages + a
 * `conversation_summaries` row directly via `testInfra.sql()`, then
 * navigates to the conversation and asserts:
 *   - the divider renders on the message whose id matches
 *     `summarized_up_to_id` (the "anchor" message),
 *   - clicking the divider expands to reveal the summary text and the
 *     model-used / updated-at line.
 *
 * The marker reads from `Stores.ConversationSummarization.current`,
 * loaded by `SummarizationStatusPill` on conversation switch +
 * `messages.size` change. Because the pill drives the load, we don't
 * need a separate API call from the spec — opening the conversation
 * triggers it.
 *
 * Note: this spec depends on the pill being mounted. If the pill is
 * ever moved out of `toolbar_status`, the marker will stop loading
 * its read-model — that's a load-bearing audit lock-in from the
 * crashed-session redo. Don't optimize.
 */

test.describe('Summarization — in-thread boundary marker', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('renders divider at the anchor message and expands to show summary', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `marker_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'conversations::read',
        'conversations::edit',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const authHeader = { Authorization: `Bearer ${userToken}` }

    // Create a conversation via REST so the active_branch_id is set up
    // exactly like a real chat (the messages-only SQL path used by the
    // backend Tier-5 tests would miss the branch wiring).
    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'marker-test' },
    })
    expect(created.ok()).toBe(true)
    const conv = await created.json()
    const branchId = conv.active_branch_id as string
    expect(typeof branchId).toBe('string')

    // Seed 5 synthetic messages (3 user, 2 assistant) directly via SQL.
    // The marker only needs `messages` + `branch_messages` +
    // `message_contents` rows + a `conversation_summaries` row pointing
    // its `summarized_up_to_id` at message #2 (the anchor).
    const messageIds: string[] = []
    for (let i = 0; i < 5; i++) {
      const role = i % 2 === 0 ? 'user' : 'assistant'
      const text =
        role === 'user'
          ? `User turn ${i}: planning a trip to Tokyo.`
          : `Assistant turn ${i}: which neighborhoods are you considering?`
      // gen_random_uuid() so the id is a string we can read back.
      const inserted = await sql(
        `INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
         VALUES (gen_random_uuid(), $1, gen_random_uuid(), 0, NOW() + ($2::int * INTERVAL '1 second'))
         RETURNING id`,
        [role, i],
      )
      const msgId = (inserted.rows[0] as { id: string }).id
      messageIds.push(msgId)
      // Branch junction — increment created_at per insert so order matches.
      await sql(
        `INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
         VALUES ($1, $2, false, NOW() + ($3::int * INTERVAL '1 second'))`,
        [branchId, msgId, i],
      )
      // Text content.
      await sql(
        `INSERT INTO message_contents (message_id, content_type, content, sequence_order)
         VALUES ($1, 'text', $2::jsonb, 0)`,
        [msgId, JSON.stringify({ type: 'text', text })],
      )
    }
    const anchorMessageId = messageIds[2]!

    // Insert the summary row — boundary is the 3rd message (index 2),
    // claiming 3 messages condensed.
    await sql(
      `INSERT INTO conversation_summaries
         (branch_id, summary_text, summarized_up_to_id, message_count, model_used)
       VALUES ($1, $2, $3, 3, 'test-model')`,
      [
        branchId,
        'The user wants to plan a Tokyo trip and is asking about neighborhoods.',
        anchorMessageId,
      ],
    )

    // Navigate to the conversation — `SummarizationStatusPill` mounts
    // in the toolbar and triggers `loadForConversation` on the
    // `conversation.id + messages.size` effect.
    await page.goto(`${baseURL}/chat/${conv.id}`)

    // Anchor message should render with the boundary divider in its
    // `message_footer` slot. The divider renders only at the anchor.
    const divider = byTestId(
      page.locator(`[data-message-id="${anchorMessageId}"]`),
      'summ-boundary-toggle',
    )
    await expect(divider).toBeVisible({ timeout: 30000 })
    // It reports the (seeded) condensed message count.
    await expect(divider).toContainText(/Earlier 3 messages condensed/i)

    // Click the divider to expand → the summary text card appears with the
    // exact seeded summary + the model/timestamp line.
    await divider.click()
    const summaryCard = byTestId(page, 'summ-boundary-card')
    await expect(summaryCard).toBeVisible({ timeout: 5000 })
    await expect(summaryCard).toContainText(
      'The user wants to plan a Tokyo trip and is asking about neighborhoods.',
    )
    await expect(summaryCard).toContainText(/Generated by test-model at/)
  })
})
