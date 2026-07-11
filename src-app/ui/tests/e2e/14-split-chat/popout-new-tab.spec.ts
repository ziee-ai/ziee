import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — pop-out into a new tab (TEST-P3 updated + TEST-66, ITEM-P2/P3 +
 * ITEM-44/DEC-60).
 *
 * The header "Open in new window/tab" (`⤢`) action opens the conversation in a
 * second top-level page (web: `window.open('/chat/<id>', 'chat-<id>')`), which
 * authenticates via the shared session and renders independently. Per DEC-60 the
 * SINGLE-pane button is desktop-only, so on the WEB it is HIDDEN in single-pane
 * and only present inside a split pane — where it also MOVES the pane out. No LLM.
 */
test.describe('Split chat — pop-out to a new tab', () => {
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

  const setup = async (
    page: import('@playwright/test').Page,
    baseURL: string,
    apiURL: string,
  ) => {
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    return token
  }

  test('web: the pop-out button is HIDDEN in single-pane, PRESENT in a split pane (DEC-60)', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    const token = await setup(page, baseURL, apiURL)
    const convA = await mkConv(page, apiURL, token, 'Popout Alpha')
    const convB = await mkConv(page, apiURL, token, 'Popout Bravo')

    // Single-pane web: NO pop-out button (the browser's own new-tab covers it).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await expect(page.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'chat-open-in-new-window')).toHaveCount(0)

    // Split → each pane's header DOES show the pop-out (its "move pane out" action).
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)
    await expect(pane0.getByTestId('chat-open-in-new-window')).toBeVisible({ timeout: 15000 })
    await expect(pane1.getByTestId('chat-open-in-new-window')).toBeVisible()
  })

  test('popping out a split pane opens an independent tab and MOVES the pane out', async ({
    page,
    context,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    const token = await setup(page, baseURL, apiURL)
    const convA = await mkConv(page, apiURL, token, 'Popout Keep')
    const convB = await mkConv(page, apiURL, token, 'Popout Me')

    // Build [A | B].
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)

    // Pop out pane B → a NEW top-level page for B opens, and pane B leaves the split.
    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      pane1.getByTestId('chat-open-in-new-window').click(),
    ])
    await popup.waitForLoadState('load')
    expect(popup.url()).toContain(`/chat/${convB}`)
    await expect(
      popup.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 30000 })

    // The origin window collapsed to single-pane A (close-to-1); B is no longer a pane.
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    await expect(page).toHaveURL(new RegExp(convA))
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible()
    await popup.close()
  })
})
