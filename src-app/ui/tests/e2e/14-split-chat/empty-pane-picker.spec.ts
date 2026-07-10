import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — the empty-pane conversation picker (TEST-49, ITEM-27 / FB-3).
 *
 * Clicking Split opens `[conversation A | an empty pane showing a searchable
 * "Open a conversation" picker]`. Typing to filter + selecting an EXISTING
 * conversation B loads B into that pane (two existing conversations side by
 * side). "Start a new chat" instead opens a new-chat composer in the pane.
 * No LLM (conversations created via the API).
 */
test.describe('Split chat — empty-pane picker', () => {
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

  test('filter + select an existing conversation loads it into the pane (two existing side by side)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Picker Primary A')
    const convB = await mkConv(page, apiURL, token, 'Picker Target Bravo')
    await mkConv(page, apiURL, token, 'Picker Noise Charlie')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // The empty pane shows the searchable picker (NOT a bare new-chat composer).
    await expect(pane1.getByTestId('conversation-picker-pane')).toBeVisible()
    await expect(pane1.getByTestId('pane-start-new-chat')).toBeVisible()
    const search = pane1.getByTestId('conversation-picker-search')
    await expect(search).toBeVisible()

    // Filter to Bravo — the Charlie noise row is filtered out.
    await search.fill('Bravo')
    await expect(pane1.getByTestId(`conversation-picker-item-${convB}`)).toBeVisible()
    await expect(pane1.getByTestId('conversation-picker-list')).not.toContainText('Charlie')

    // Selecting B loads it into pane 1 — two EXISTING conversations side by side.
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    // The picker is gone; the pane now renders conversation B's composer.
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)
    await expect(
      pane1.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })
    // Pane 0 still holds conversation A (untouched).
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
  })

  test('"Start a new chat" opens a new-chat composer in the pane', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Picker NewChat A')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await expect(pane1.getByTestId('conversation-picker-pane')).toBeVisible()

    // "Start a new chat" switches the pane to the new-chat greeting + composer.
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()
    await expect(
      pane1.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })
    // A back affordance returns to the browse list.
    await pane1.getByTestId('pane-picker-back').click()
    await expect(pane1.getByTestId('conversation-picker-search')).toBeVisible()
  })
})
