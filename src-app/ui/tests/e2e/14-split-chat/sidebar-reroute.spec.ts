import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — sidebar reroute (TEST-50, ITEM-28). With a split open, a plain
 * sidebar click FOCUSES a conversation already in a pane (no duplicate) or
 * REPLACES the focused pane's conversation (no full navigate that tears down the
 * split); Cmd/Ctrl/middle-click opens it in a NEW pane. The recent-conversations
 * widget and the history card both funnel through the same
 * `useOpenConversationInWorkspace` hook. No LLM.
 */
test.describe('Split chat — sidebar reroute', () => {
  const mkConv = async (
    page: import('@playwright/test').Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  test('plain click replaces the focused pane or focuses an open one; modifier click opens a new pane', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Reroute Alpha')
    const convB = await mkConv(page, apiURL, token, 'Reroute Bravo')
    const convC = await mkConv(page, apiURL, token, 'Reroute Charlie')

    // Build a [A | B] split: view A, open B in split via the recent-row ⋯-menu.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 20000 })
    await rowB.hover()
    await byTestId(page, `chat-recent-row-actions-btn-${convB}`).click()
    await byTestId(page, `chat-recent-row-menu-${convB}-item-open-in-split`).click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })

    // Focus pane 0 (conv A), then plain-click C (not open) → REPLACE pane 0 with C.
    await byTestId(page, 'chat-pane-0').click()
    await expect(byTestId(page, 'chat-pane-0')).toHaveClass(/ring-primary/)
    await byTestId(page, `chat-recent-conversations-menu-item-${convC}`).click()
    // Still exactly 2 panes (split NOT collapsed); focused pane 0 now shows C.
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-0').getByTestId('conversation-title')).toContainText(
      'Charlie',
    )

    // Plain-click B (open in pane 1) → FOCUS pane 1 (dedupe, no third pane).
    await byTestId(page, `chat-recent-conversations-menu-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-1')).toHaveClass(/ring-primary/)
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)

    // Cmd/Ctrl-click A (not currently open — pane 0 holds C now) → NEW pane (3rd).
    await byTestId(page, `chat-recent-conversations-menu-item-${convA}`).click({
      modifiers: ['ControlOrMeta'],
    })
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
  })
})
