import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * TEST-10 (feature: lazy-load-conversation-messages) — jump to a possibly-
 * unloaded message via the `#message-<id>` deep-link, then scroll DOWN to load
 * newer ("load more around the found message").
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

test.describe('Chat — jump to message (deep-link)', () => {
  test('deep-link centers + highlights an unloaded message and pages newer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Jump-to test')

    const all = Array.from({ length: 40 }, (_, i) =>
      mockUserMessage({ id: `msg-${i}`, text: `Message number ${i}` }),
    )
    await mockPaginatedMessages(page, all, { pageSize: 30 })

    // Deep-link to msg-20 — neither the tail (msg-39) nor the head (msg-0) is in
    // an around-window centered on it.
    await page.goto(`${baseURL}/chat/${convId}#message-msg-20`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // The target loaded, is visible, and carries the find highlight ring.
    await expect(page.locator('[data-message-id="msg-20"]')).toBeVisible({
      timeout: 10000,
    })
    await expect(
      page.locator('[data-message-id="msg-20"][data-find-active]'),
    ).toBeVisible()

    // The real tail is NOT loaded (window is centered mid-conversation).
    await expect(page.locator('[data-message-id="msg-39"]')).toHaveCount(0)

    // Scroll DOWN toward the bottom-load sentinel → newer messages load.
    await page.getByTestId('chat-bottom-load-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="msg-39"]')).toBeVisible({
      timeout: 10000,
    })
  })
})
