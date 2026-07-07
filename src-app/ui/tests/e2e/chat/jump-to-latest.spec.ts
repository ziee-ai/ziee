import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockGetMessages,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * ITEM-2 / TEST-9 — jump-to-latest button.
 *
 * In a long conversation the button is hidden when at the bottom, appears after
 * scrolling up, and returns the view to the latest message on click.
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

function msg(
  id: string,
  role: 'user' | 'assistant',
  text: string,
): MockMessageWithContent {
  return { id, role, contents: [{ content_type: 'text', content: { type: 'text', text } }] }
}

test.describe('Chat — jump to latest', () => {
  test('shows the jump button when scrolled up and returns to the bottom', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Jump test')

    // Many tall messages so the list overflows and can be scrolled.
    const filler = Array.from({ length: 20 }, (_, i) => `Paragraph ${i}. `).join('\n')
    const messages = Array.from({ length: 16 }, (_, i) =>
      msg(`jm${i}`, i % 2 === 0 ? 'user' : 'assistant', `Message ${i}\n${filler}`),
    )
    await mockGetMessages(page, messages)

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    const jumpBtn = page.getByTestId('chat-jump-to-latest-btn')

    // Initial load jumps to the bottom → button hidden.
    await expect(jumpBtn).toBeHidden()

    // Scroll up to the first message → button appears.
    await page.locator('[data-message-id="jm0"]').scrollIntoViewIfNeeded()
    await expect(jumpBtn).toBeVisible()

    // Click → returns to the latest message and hides again.
    await jumpBtn.click()
    await expect(jumpBtn).toBeHidden()
  })
})
