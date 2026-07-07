import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockGetMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * ITEM-1 / TEST-8 — find-within-open-conversation.
 *
 * Seeds a conversation (REST) and renders deterministic messages via the mocked
 * getHistory endpoint, then drives the find bar: open, type a term present in
 * two messages, assert the "X of Y" readout, and that Next moves the active
 * highlight to the next match. Only the message-history boundary is mocked; the
 * find UI runs for real.
 */

async function seedConversation(
  apiURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status} ${await res.text()}`)
  return (await res.json()).id as string
}

test.describe('Chat — find in conversation', () => {
  test('finds matches, shows count, and navigates between them', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Find test conversation')

    await mockGetMessages(page, [
      mockUserMessage({ id: 'fm1', text: 'The first message talks about apples' }),
      mockUserMessage({ id: 'fm2', text: 'The second message is about oranges' }),
      mockUserMessage({ id: 'fm3', text: 'A third message mentions apples again' }),
    ])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Open the find bar via the header toggle.
    await page.getByTestId('conversation-find-toggle-btn').click()
    const input = page.getByTestId('conversation-find-input')
    await expect(input).toBeVisible()

    await input.fill('apples')

    // Two matches; the first is active.
    await expect(page.getByTestId('conversation-find-count')).toHaveText('1 of 2')
    await expect(page.locator('[data-message-id="fm1"][data-find-active]')).toBeVisible()

    // Next → second match becomes active.
    await page.getByTestId('conversation-find-next').click()
    await expect(page.getByTestId('conversation-find-count')).toHaveText('2 of 2')
    await expect(page.locator('[data-message-id="fm3"][data-find-active]')).toBeVisible()

    // A non-matching term reports no results.
    await input.fill('zzz-not-here')
    await expect(page.getByTestId('conversation-find-count')).toHaveText('No results')

    // Esc closes the bar.
    await input.press('Escape')
    await expect(input).toBeHidden()
  })
})
