import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * TEST-3 (feature: virtualize-conversation-messages) — the message list is
 * row-virtualized: a long LOADED window mounts only the visible messages +
 * overscan, and scrolling changes WHICH messages are mounted, while the scroll
 * height is preserved. A large page size loads the whole set in one page so
 * there's no lazy-load pagination noise — the reduction is purely virtualization.
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

test.describe('Chat — virtualized message list', () => {
  test('mounts only visible rows and re-windows on scroll', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Virtualization test')

    // 60 messages, all loaded in ONE page (large page size → no pagination).
    const all = Array.from({ length: 60 }, (_, i) =>
      mockUserMessage({ id: `v-${i}`, text: `Virtual message number ${i}` }),
    )
    await mockPaginatedMessages(page, all, { pageSize: 200 })

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Initial (bottom): newest rendered, and only a SUBSET of the 60 loaded
    // messages is actually mounted — proof of virtualization.
    await expect(page.locator('[data-message-id="v-59"]')).toBeVisible()
    const mounted = await page.getByTestId('chat-message').count()
    expect(mounted).toBeGreaterThan(0)
    expect(mounted).toBeLessThan(30) // « 60 loaded
    // The oldest message is loaded but virtualized OUT at the bottom.
    await expect(page.locator('[data-message-id="v-0"]')).toHaveCount(0)

    // Scroll to the very top (via the always-mounted top sentinel) → the window
    // shifts: oldest mounts, newest unmounts. (No pagination — hasMoreBefore is
    // false, so this only scrolls.)
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="v-0"]')).toBeVisible({
      timeout: 10000,
    })
    await expect(page.locator('[data-message-id="v-59"]')).toHaveCount(0)

    // Still only a subset mounted after the re-window.
    const mountedTop = await page.getByTestId('chat-message').count()
    expect(mountedTop).toBeLessThan(30)
  })
})
