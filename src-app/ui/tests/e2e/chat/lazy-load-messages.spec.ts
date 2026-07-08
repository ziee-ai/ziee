import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * TEST-9 (feature: lazy-load-conversation-messages) — recent-first load +
 * reverse-infinite-scroll with scroll anchoring.
 *
 * A long conversation loads only the newest page; older messages are NOT in the
 * DOM until the user scrolls to the top, at which point they prepend WITHOUT
 * teleporting the viewport (the previously-visible content stays put). Only the
 * paginated message-history boundary is mocked; the store windowing + scroll
 * observers + anchor restore run for real.
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

test.describe('Chat — lazy-load messages', () => {
  test('loads recent first, prepends older on scroll-up, preserves scroll position', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Lazy-load test')

    // 40 messages; page size 30 → tail loads msg-10..msg-39, older stay unloaded.
    const all = Array.from({ length: 40 }, (_, i) =>
      mockUserMessage({ id: `msg-${i}`, text: `Message number ${i}` }),
    )
    await mockPaginatedMessages(page, all, { pageSize: 30 })

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Newest present, oldest NOT yet loaded (only the tail page rendered).
    await expect(page.locator('[data-message-id="msg-39"]')).toBeVisible()
    await expect(page.locator('[data-message-id="msg-0"]')).toHaveCount(0)
    await expect(page.locator('[data-message-id="msg-9"]')).toHaveCount(0)

    // Reference: the oldest currently-loaded message (msg-10) — record its
    // viewport position before triggering the older-page load.
    const refBefore = await page
      .locator('[data-message-id="msg-10"]')
      .boundingBox()
    expect(refBefore).not.toBeNull()

    // Scroll to the top sentinel → triggers loadOlderMessages (prepend).
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()

    // Older messages arrive.
    await expect(page.locator('[data-message-id="msg-0"]')).toBeVisible({
      timeout: 10000,
    })

    // Scroll anchoring: the reference message must not have teleported — its
    // viewport y stays within a small tolerance despite content prepended above.
    const refAfter = await page
      .locator('[data-message-id="msg-10"]')
      .boundingBox()
    expect(refAfter).not.toBeNull()
    expect(Math.abs(refAfter!.y - refBefore!.y)).toBeLessThan(80)
  })
})
