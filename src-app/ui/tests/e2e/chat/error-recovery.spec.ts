import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — chat error-RECOVERY paths.
 *
 *  1. Conversation-list load failure surfaces a persistent, retryable
 *     <ErrorState> (`ConversationList.tsx`, fed by `ChatHistory.store` setting
 *     `error: 'Failed to load conversations'`); "Try again" re-fetches and
 *     recovers.
 *  2. A failed regenerate (the underlying send POST 500s) must not leave the
 *     composer stuck "streaming" — `Chat.store.sendMessage`'s catch clears
 *     `sending`/`isStreaming`, so the Send button re-enables
 *     (`MessageActions.handleRegenerate` → `startRegenerateMessage`).
 */

test.describe('Chat — error recovery', () => {
  test('a failed conversations load shows a persistent error state that retries', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Fail the conversations list GET from the start so the first
    // loadConversations() flips the store into its error state.
    const failRoute = /\/api\/conversations(\?.*)?$/
    await page.route(failRoute, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'boom' } }),
        })
      }
      return route.fallback()
    })

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/chats`)

    // The conversation list surfaces a persistent ErrorState (not toast-only).
    await expect(byTestId(page, 'chat-history-error').first()).toBeVisible({
      timeout: 30000,
    })

    // "Try again" re-fetches. Stop failing the GET, then retry → the error
    // state clears and the list recovers (empty, no conversations seeded).
    await page.unroute(failRoute)
    await byTestId(page, 'chat-history-error-retry').first().click()
    await expect(byTestId(page, 'chat-history-error')).toHaveCount(0, {
      timeout: 10000,
    })
  })

  test('a failed regenerate recovers the composer (no stuck streaming)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed a conversation with one user + one assistant message (SQL, mirroring
    // summarization/in-thread-marker) so the transcript renders a
    // Regenerate button on the assistant bubble.
    const created = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'regen-error-recovery' },
    })
    expect(created.ok()).toBeTruthy()
    const conv = await created.json()
    const branchId = conv.active_branch_id as string

    for (let i = 0; i < 2; i++) {
      const role = i === 0 ? 'user' : 'assistant'
      const text = role === 'user' ? 'Tell me a joke.' : 'Why did the chicken…'
      const inserted = await sql(
        `INSERT INTO messages (id, role, originated_from_id, edit_count, created_at)
         VALUES (gen_random_uuid(), $1, gen_random_uuid(), 0, NOW() + ($2::int * INTERVAL '1 second'))
         RETURNING id`,
        [role, i],
      )
      const msgId = (inserted.rows[0] as { id: string }).id
      await sql(
        `INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
         VALUES ($1, $2, false, NOW() + ($3::int * INTERVAL '1 second'))`,
        [branchId, msgId, i],
      )
      await sql(
        `INSERT INTO message_contents (message_id, content_type, content, sequence_order)
         VALUES ($1, 'text', $2::jsonb, 0)`,
        [msgId, JSON.stringify({ type: 'text', text })],
      )
    }

    // Force the regenerate's underlying send to fail.
    await page.route(/\/api\/conversations\/[^/]+\/messages$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'send failed' } }),
        })
      }
      return route.fallback()
    })

    await page.goto(`${baseURL}/chat/${conv.id}`)
    await page.waitForSelector('[data-role="assistant"]', { timeout: 30000 })

    // Regenerate the assistant message → the send 500s.
    await page.locator('[data-testid="regenerate-button"]').first().click()

    // Recovery: the composer is not stuck in a streaming state — the Send
    // button returns to enabled once sendMessage's catch clears the flags.
    await expect(byTestId(page, 'chat-input-send-btn')).toBeEnabled({
      timeout: 30000,
    })
  })
})
