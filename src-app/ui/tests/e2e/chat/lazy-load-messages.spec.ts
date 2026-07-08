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

    // Scroll up INTO the top-sentinel trigger zone and capture the pre-prepend
    // scroll geometry in the same tick. `findScroller` runs in the browser and
    // locates the nearest scrollable ancestor of the message list (the
    // OverlayScrollbars viewport on desktop).
    const before = await page.evaluate(() => {
      const findScroller = (): HTMLElement => {
        const list = document.querySelector('[data-testid="chat-messages"]')
        let n: HTMLElement | null = list?.parentElement ?? null
        while (n) {
          const s = getComputedStyle(n)
          if (
            (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
            n.scrollHeight > n.clientHeight
          ) {
            return n
          }
          n = n.parentElement
        }
        return document.scrollingElement as HTMLElement
      }
      const vp = findScroller()
      vp.scrollTop = 200
      return { scrollTop: vp.scrollTop, scrollHeight: vp.scrollHeight }
    })

    // Wait for the older page to prepend + the anchor restore to settle. (Under
    // virtualization the newly-prepended msg-0 is loaded but OFF-SCREEN — the
    // anchor correctly keeps the prior content in view — so we don't assert it's
    // visible; we assert the scroll invariant below.)
    await page.waitForResponse(
      r => /\/messages\?[^ ]*before=/.test(r.url()) && r.status() === 200,
      { timeout: 10000 },
    )
    await page.waitForTimeout(800)

    // Scroll-anchor invariant: the viewport's scrollTop grew by (about) the same
    // amount as the content that was prepended above it — so the previously
    // visible content stayed put instead of teleporting. Without anchoring,
    // scrollTop would be unchanged (~200) and the delta would be the full
    // prepended height.
    const after = await page.evaluate(() => {
      const list = document.querySelector('[data-testid="chat-messages"]')
      let n: HTMLElement | null = list?.parentElement ?? null
      while (n) {
        const s = getComputedStyle(n)
        if (
          (s.overflowY === 'auto' || s.overflowY === 'scroll') &&
          n.scrollHeight > n.clientHeight
        ) {
          break
        }
        n = n.parentElement
      }
      const vp = (n ?? document.scrollingElement) as HTMLElement
      return { scrollTop: vp.scrollTop, scrollHeight: vp.scrollHeight }
    })

    const addedAbove = after.scrollHeight - before.scrollHeight
    const scrollGrew = after.scrollTop - before.scrollTop
    expect(addedAbove).toBeGreaterThan(50) // content really was prepended
    expect(Math.abs(scrollGrew - addedAbove)).toBeLessThan(80) // anchored
  })
})
