import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — deleting a conversation that is open in a pane CLOSES that pane
 * (TEST-112, ITEM-74 / FB-23). Regression: `deleteConversation` emits a LOCAL
 * `conversation.deleted` EventBus event, but SplitView only listened to the
 * cross-device `sync:conversation` SSE event (whose self-echo is suppressed for the
 * deleting device) — so deleting from the sidebar left the pane STALE, still showing
 * the deleted conversation. SplitView now also listens to `conversation.deleted`.
 *
 * Drives the exact repro: open A|B, delete B from the sidebar ⋯ menu → the B pane
 * closes and the workspace collapses to single-pane A. No LLM.
 */
test.describe('Split chat — deleting a conversation open in a pane closes that pane', () => {
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

  test('deleting convB from the sidebar closes pane B; workspace collapses to single-pane A', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Del Alpha')
    const convB = await mkConv(page, apiURL, token, 'Del Bravo')

    // Build [A | B] (split button + picker).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1').getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-1').getByTestId('conversation-title')).toContainText('Bravo', { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()

    // Delete convB from the sidebar ⋯ menu (the LOCAL delete path).
    await byTestId(page, `chat-recent-row-actions-btn-${convB}`).click({ force: true })
    await byTestId(page, `chat-recent-row-menu-${convB}-item-delete`).click()
    await byTestId(page, 'chat-conversation-delete-confirm-btn').click()

    // The pane holding B closes → single-pane A (no split, no second pane).
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0, { timeout: 15000 })
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    // The surviving view is conversation A, not the deleted B.
    await expect(byTestId(page, 'conversation-title')).toContainText('Alpha', { timeout: 15000 })
    // ...and the URL followed to the survivor A — NOT left on the deleted B (B was
    // the focused pane, so its URL was `/chat/B`; leaving it there makes the
    // single-pane view load a gone conversation and toast "does not exist" — FB-25).
    await expect(page).toHaveURL(new RegExp(`/chat/${convA}$`), { timeout: 15000 })
  })
})
