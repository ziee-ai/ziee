import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockGetMessages,
  mockUserMessage,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * ITEM-3 / TEST-10 — collapse long messages by default.
 *
 * A very long message renders clamped with a "Show more" control; clicking it
 * expands, and "Show less" re-clamps. A short message shows no toggle.
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

// A message long enough to overflow the ~24rem clamp: many lines, > threshold.
const LONG_TEXT = Array.from({ length: 80 }, (_, i) => `Line ${i + 1} of a very long message that keeps going and going.`).join('\n')

function longAssistant(id: string): MockMessageWithContent {
  return {
    id,
    role: 'assistant',
    contents: [{ content_type: 'text', content: { type: 'text', text: LONG_TEXT } }],
  }
}

test.describe('Chat — collapse long messages', () => {
  test('clamps a long message with a Show more/less toggle', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Collapse test')

    await mockGetMessages(page, [
      mockUserMessage({ id: 'short1', text: 'a short message' }),
      longAssistant('long1'),
    ])

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // The long message is wrapped in a collapsible with a visible "Show more".
    const toggle = page.getByTestId('collapsible-toggle')
    await expect(toggle).toBeVisible()
    await expect(toggle).toHaveText(/Show more/i)

    // The collapsed content is clamped.
    const block = page.getByTestId('chat-message-collapsible')
    await expect(block).toBeVisible()

    // Expand → "Show less".
    await toggle.click()
    await expect(toggle).toHaveText(/Show less/i)

    // Re-clamp.
    await toggle.click()
    await expect(toggle).toHaveText(/Show more/i)
  })
})
