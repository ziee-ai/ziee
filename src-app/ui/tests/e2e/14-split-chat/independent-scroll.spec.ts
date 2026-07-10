import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * Split-chat E2E — per-pane scroll + virtualization independence (TEST-16 /
 * TEST-39 / ITEM-2/7). Each pane owns its own DivScrollY viewport + virtualizer +
 * top-sentinel, so a large history virtualizes in pane A and scrolling pane A
 * leaves pane B untouched. Messages are mocked (route interception) — no LLM.
 */
const BODY = (i: number) =>
  `Message number ${i}. ` + 'Lorem ipsum dolor sit amet. '.repeat(6)

async function seedConversation(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
  title: string,
) {
  // Use Playwright's request context (not node `fetch`) to match the sibling
  // specs' seeding idiom (audit LOW, a50e/patterns).
  const res = await page.request.post(`${apiURL}/api/conversations`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { title },
  })
  if (res.status() >= 300) throw new Error(`seed failed: ${res.status()}`)
  return (await res.json()).id as string
}

test.describe('Split chat — per-pane scroll + virtualization', () => {
  test("pane A virtualizes a long history and scrolls without moving pane B", async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await seedConversation(page, apiURL, token, 'Scroll Pane A')

    // 30 messages (one page). Mocked for every /messages fetch — only pane A
    // (conv A) fetches; the new-chat pane B fetches none.
    const all = Array.from({ length: 30 }, (_, i) =>
      mockUserMessage({ id: `v-${i}`, text: BODY(i) }),
    )
    await mockPaginatedMessages(page, all)

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Open the split: pane 0 = conv A (long history), pane 1 = a new-chat pane.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0.getByTestId('chat-messages')).toBeVisible({ timeout: 30000 })
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Pane 0 virtualizes: newest mounted, oldest virtualized OUT, only a subset
    // of the 30 messages in the DOM.
    await expect(pane0.locator('[data-message-id="v-29"]')).toBeVisible()
    await expect(pane0.locator('[data-message-id="v-0"]')).toHaveCount(0)
    const mounted0 = await pane0.getByTestId('chat-message').count()
    expect(mounted0).toBeGreaterThan(0)
    expect(mounted0).toBeLessThan(20)

    // Pane 1 (new-chat) has no messages at all.
    await expect(pane1.getByTestId('chat-message')).toHaveCount(0)

    // Scroll pane 0 to the top via ITS OWN top sentinel → pane 0's window shifts
    // (oldest mounts, newest unmounts).
    await pane0.getByTestId('chat-top-sentinel').scrollIntoViewIfNeeded()
    await expect(pane0.locator('[data-message-id="v-0"]')).toBeVisible({
      timeout: 10000,
    })
    await expect(pane0.locator('[data-message-id="v-29"]')).toHaveCount(0)

    // Pane 1 is completely unaffected by pane 0's scroll/pagination.
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()
    await expect(pane1.getByTestId('chat-message')).toHaveCount(0)
  })
})
