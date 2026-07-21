import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — BULK-deleting a conversation open in a pane closes that pane (#168).
 *
 * Sibling of `conversation-delete-closes-pane.spec.ts`, which covers the single-delete
 * path. `ChatHistory.bulkDelete` never emitted `conversation.deleted` at all (only
 * `deleteConversation` did), so the multi-select delete on `/chats` left every pane
 * holding a bulk-deleted conversation stale — and left nothing able to navigate an open
 * `/chat/:id` off a dead id. `bulkDelete` now broadcasts per id.
 *
 * Builds [A | B], bulk-deletes B from `/chats`, then returns to the workspace and
 * asserts pane B is gone. No LLM.
 */
test.describe('Split chat — bulk-deleting a conversation open in a pane closes that pane', () => {
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

  test('bulk-deleting convB from /chats closes pane B', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Bulk Alpha')
    const convB = await mkConv(page, apiURL, token, 'Bulk Bravo')

    // Build [A | B] (split button + picker).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1')
      .getByTestId(`conversation-picker-item-${convB}`)
      .click()
    await expect(
      byTestId(page, 'chat-pane-1').getByTestId('conversation-title'),
    ).toContainText('Bravo', { timeout: 15000 })

    // Bulk-delete B from the /chats list (the path that emitted nothing before).
    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('domcontentloaded')
    const cardB = byTestId(page, `chat-conversation-card-${convB}`)
    await expect(cardB).toBeVisible({ timeout: 15000 })
    await cardB.hover()
    await byTestId(page, `chat-conversation-select-${convB}`).click()
    await byTestId(page, 'chat-bulk-delete-btn').click()
    await byTestId(page, 'chat-bulk-delete-confirm-confirm').click()
    await expect(cardB).toHaveCount(0, { timeout: 15000 })

    // Back to the workspace: pane B is gone, A survives — not a stale second pane
    // still rendering the deleted Bravo.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'conversation-title')).toContainText('Alpha', {
      timeout: 15000,
    })
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)
    await expect(page.getByText('Bulk Bravo')).toHaveCount(0)
  })
})
