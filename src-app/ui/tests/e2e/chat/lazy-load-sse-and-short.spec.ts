import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockChatTokenStream,
  mockPaginatedMessages,
  mockUserMessage,
} from '../helpers/sse-mock-helpers'

/**
 * TEST-12 (feature: lazy-load-conversation-messages):
 *  (a) sending a new turn APPENDS at the bottom without discarding loaded older
 *      pages (the reconcile-tail merge path); and
 *  (b) a SHORT conversation (< page size) renders every message with no top
 *      spinner and never fires a `before=` pagination request.
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

test.describe('Chat — lazy-load SSE + short conversation', () => {
  test('(b) short conversation renders fully and never paginates', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Short conv')

    const all = Array.from({ length: 5 }, (_, i) =>
      mockUserMessage({ id: `s-${i}`, text: `Short message ${i}` }),
    )
    const { queries } = await mockPaginatedMessages(page, all, { pageSize: 30 })

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Every message present; no top loading affordance.
    await expect(page.locator('[data-message-id="s-0"]')).toBeVisible()
    await expect(page.locator('[data-message-id="s-4"]')).toBeVisible()
    await expect(page.getByTestId('chat-loading-older')).toBeEmpty()

    // Scrolling to the top must NOT trigger an older-page fetch.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await page.waitForTimeout(500)
    expect(queries.some(q => q.includes('before='))).toBe(false)
  })

  test('(a) a new turn appends at the bottom, keeping earlier messages', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'SSE append')

    // A live message list the GET mock serves; the POST/stream mock pushes the
    // new turn so reconcile-tail (fired on `complete`) returns it too.
    const all = Array.from({ length: 6 }, (_, i) =>
      mockUserMessage({ id: `h-${i}`, text: `History message ${i}` }),
    )

    // Stream mock: one send → user id `umsg_0`, an assistant text reply.
    await mockChatTokenStream(page, [
      [
        { event: 'started', data: { user_message_id: 'umsg_0' } },
        {
          event: 'content',
          data: {
            message_id: 'amsg_0',
            content: [{ type: 'text_delta', delta: 'Hello from the assistant reply' }],
          },
        },
        { event: 'complete', data: { message_id: 'amsg_0' } },
      ],
    ])
    // When the send is accepted, add the finalized user+assistant to the served
    // history so the post-complete reconcile-tail keeps them.
    page.on('request', req => {
      if (
        req.method() === 'POST' &&
        /\/conversations\/[^/]+\/messages(\?|$)/.test(req.url())
      ) {
        all.push(
          mockUserMessage({ id: 'umsg_0', text: 'A brand new question' }),
        )
        all.push({
          id: 'amsg_0',
          role: 'assistant',
          contents: [
            {
              content_type: 'text',
              content: { type: 'text', text: 'Hello from the assistant reply' },
            },
          ],
        })
      }
    })
    // Register the GET mock AFTER the stream mock so GET is served here and POST
    // falls back to the stream mock.
    await mockPaginatedMessages(page, all, { pageSize: 30 })

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })
    await expect(page.locator('[data-message-id="h-0"]')).toBeVisible()

    // Send a message.
    const composer = page.getByTestId('chat-input-textarea')
    await composer.fill('A brand new question')
    await composer.press('Enter')

    // The streamed reply appears at the bottom, and the earliest history message
    // is still present (older content not discarded by the new turn).
    await expect(page.getByText('Hello from the assistant reply')).toBeVisible({
      timeout: 15000,
    })
    await expect(page.locator('[data-message-id="h-0"]')).toBeVisible()
  })
})
