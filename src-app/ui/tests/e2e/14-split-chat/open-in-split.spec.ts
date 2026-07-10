import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — open an EXISTING conversation into a second pane (TEST-22,
 * ITEM-28 / ITEM-25). From conversation A, the sidebar recent-row ⋯-menu "Open in
 * split pane" for a DIFFERENT conversation B puts two EXISTING conversations side
 * by side (intent:'newPane') and focuses the new pane. No LLM.
 */
test.describe('Split chat — open existing conversation in split', () => {
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

  test('the recent-row ⋯-menu "Open in split pane" opens A|B side by side, focusing the new pane', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Split Source Alpha')
    const convB = await mkConv(page, apiURL, token, 'Split Target Bravo')

    // View conversation A (single pane) — currentConversationId resolves from the
    // path, so opening B "in split" produces a 2-pane [A | B] workspace.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0) // single pane

    // The sidebar recent-conversations row for B; hover to reveal its ⋯ button.
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 20000 })
    await rowB.hover()
    await byTestId(page, `chat-recent-row-actions-btn-${convB}`).click()
    // "Open in split pane" (intent:'newPane').
    await byTestId(page, `chat-recent-row-menu-${convB}-item-open-in-split`).click()

    // A 2-pane split: pane 0 = A, pane 1 = B, and pane 1 (the new pane) is focused.
    await expect(byTestId(page, 'split-chat-view')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-1')).toHaveClass(/ring-primary/)
    // The URL tracks the focused (new) pane → conversation B.
    await expect(page).toHaveURL(new RegExp(`/chat/${convB}$`))
  })
})
