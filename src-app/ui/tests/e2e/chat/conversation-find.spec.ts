import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockConversationSearch,
  mockPaginatedMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'

/**
 * TEST-16 (feature: lazy-load-conversation-messages) — server-side
 * find-in-conversation. Under lazy-load the client holds only a WINDOW, so find
 * queries the backend; a match in an UNLOADED older message still surfaces in
 * the results list, and selecting it JUMPS to the message (around=), centering +
 * highlighting it. Only the search + paginated-history boundaries are mocked;
 * the find UI + jump + windowing run for real.
 */

async function seedConversation(apiURL: string, token: string, title: string) {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status}`)
  return (await res.json()).id as string
}

test.describe('Chat — find in conversation (server-side)', () => {
  test('surfaces a match in an unloaded message and jumps to it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Find test conversation')

    // 40 messages; only msg-5 (unloaded — outside the tail window) contains the
    // unique term.
    const all = Array.from({ length: 40 }, (_, i) =>
      mockUserMessage({
        id: `msg-${i}`,
        text: i === 5 ? 'a very special-marker message' : `Message number ${i}`,
      }),
    )
    await mockPaginatedMessages(page, all, { pageSize: 30 })
    await mockConversationSearch(page, [
      { message_id: 'msg-5', snippet: 'a very special-marker message' },
    ])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // The match's message is NOT loaded initially.
    await expect(page.locator('[data-message-id="msg-5"]')).toHaveCount(0)

    // Open find + search the unique term.
    await page.getByTestId('conversation-find-toggle-btn').click()
    const input = page.getByTestId('conversation-find-input')
    await expect(input).toBeVisible()
    await input.fill('special-marker')

    // Server-backed count + a results-list row with the snippet.
    await expect(page.getByTestId('conversation-find-count')).toHaveText('1 of 1')
    await expect(page.getByTestId('conversation-find-result')).toContainText(
      'special-marker',
    )

    // Activating the match jumped to it (around=): now loaded, centered, and
    // highlighted with the find ring.
    await expect(page.locator('[data-message-id="msg-5"]')).toBeVisible({
      timeout: 10000,
    })
    await expect(
      page.locator('[data-message-id="msg-5"][data-find-active]'),
    ).toBeVisible()

    // A non-matching term reports no results.
    await input.fill('zzz-not-here')
    await expect(page.getByTestId('conversation-find-count')).toHaveText('No results')

    // Esc closes.
    await input.press('Escape')
    await expect(input).toBeHidden()
  })
})
