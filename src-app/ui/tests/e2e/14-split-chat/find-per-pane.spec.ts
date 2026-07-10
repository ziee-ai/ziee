import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { mockPaginatedMessages, mockUserMessage } from '../helpers/sse-mock-helpers'

/**
 * Split-chat E2E — the find bar is pane-scoped (TEST-43). Each pane owns its own
 * `findOpen` state + `ConversationFindBar`, so toggling find in one pane opens it
 * there ONLY — the other pane's find bar stays closed. No LLM (messages mocked).
 */
test.describe('Split chat — per-pane find bar', () => {
  test('toggling find in pane A opens the find bar in pane A only', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Find Pane A' },
    })
    const convA = (await res.json()).id as string

    // A few messages so the find surface has content to search.
    await mockPaginatedMessages(
      page,
      Array.from({ length: 5 }, (_, i) =>
        mockUserMessage({ id: `f-${i}`, text: `findable message ${i}` }),
      ),
    )

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // Neither find bar is open initially.
    await expect(pane0.getByTestId('conversation-find-bar')).toHaveCount(0)
    await expect(pane1.getByTestId('conversation-find-bar')).toHaveCount(0)

    // Toggle find in pane 0 (the conversation pane).
    await pane0.getByTestId('conversation-find-toggle-btn').click()

    // The find bar + input open in pane 0 ONLY.
    await expect(pane0.getByTestId('conversation-find-bar')).toBeVisible()
    await expect(pane0.getByTestId('conversation-find-input')).toBeVisible()
    // Pane 1 (a fresh new-chat pane) has no find bar — the toggle is pane-scoped.
    await expect(pane1.getByTestId('conversation-find-bar')).toHaveCount(0)
  })
})
