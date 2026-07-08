import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * TEST-3 (feature: virtualize-conversation-messages) — the message list is
 * row-virtualized: a fully-loaded conversation mounts only the visible rows +
 * overscan, and scrolling changes WHICH rows are mounted, while the scroll
 * height is preserved.
 *
 * Uses exactly ONE page (30 tall messages) so `has_more_before` is false and
 * scrolling to the top does NOT paginate (a loadOlder + anchor-restore would
 * correctly KEEP the scroll position rather than jump to the oldest) — the
 * reduction + re-windowing here are purely virtualization.
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

// A multi-line body so each row is tall (~120px+) → few fit the viewport → the
// virtualization reduction is unambiguous.
const BODY = (i: number) =>
  `Virtual message number ${i}. ` + `This is a longer body line. `.repeat(6)

test.describe('Chat — virtualized message list', () => {
  test('mounts only visible rows and re-windows on scroll', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Virtualization test')

    // 30 messages = exactly one page (the store requests limit=30) → all loaded,
    // has_more_before false, no pagination on scroll-up.
    const all = Array.from({ length: 30 }, (_, i) =>
      mockUserMessage({ id: `v-${i}`, text: BODY(i) }),
    )
    await mockPaginatedMessages(page, all)

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })

    // Initial (bottom): newest rendered, only a SUBSET of the 30 loaded messages
    // is mounted — proof of virtualization.
    await expect(page.locator('[data-message-id="v-29"]')).toBeVisible()
    const mounted = await page.getByTestId('chat-message').count()
    expect(mounted).toBeGreaterThan(0)
    expect(mounted).toBeLessThan(20) // « 30 loaded (tall rows → few fit)
    // Oldest is loaded but virtualized OUT at the bottom.
    await expect(page.locator('[data-message-id="v-0"]')).toHaveCount(0)

    // Scroll to the very top (via the always-mounted top sentinel) → the window
    // shifts: oldest mounts, newest unmounts. No pagination (has_more_before
    // false), so this only scrolls.
    await page.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(page.locator('[data-message-id="v-0"]')).toBeVisible({
      timeout: 10000,
    })
    await expect(page.locator('[data-message-id="v-29"]')).toHaveCount(0)
    expect(await page.getByTestId('chat-message').count()).toBeLessThan(20)
  })
})
