import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — keyboard affordances act on the FOCUSED pane (TEST-9 / TEST-20).
 * Pressing Enter to send targets the composer of the pane the user is interacting
 * with (the focused pane), NOT the first pane in the DOM — proving the
 * `Stores.Chat` bridge + keyboard handling are pane-scoped by focus. Real send via
 * the local bridge; skips cleanly with no bridge configured.
 */
test.describe('Split chat — focused-pane keyboard', () => {
  test('Enter sends in the FOCUSED pane (pane B), leaving the first pane (A) idle', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Focused A' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Split: pane 0 = conv A (first in DOM), pane 1 = a fresh new-chat pane.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })
    // pane 1 opens the conversation PICKER (ITEM-27); "Start a new chat" reaches
    // the new-chat composer so pane 1 has a real composer to focus + send from.
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Focus the SECOND pane, type, and press Enter (no Send-button click).
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await expect(inputB).toBeVisible({ timeout: 15000 })
    await inputB.click() // focus pane 1
    await inputB.fill('Reply with exactly the single word: PONG')
    await inputB.press('Enter')

    // The keyboard send landed in pane 1 (the focused pane): a user message +
    // then a streamed assistant reply appear THERE.
    await expect(pane1.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(pane1.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })

    // Pane 0 (first in the DOM, NOT focused) received nothing — the keyboard was
    // pane-scoped by focus, not by DOM order.
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(0)
    await expect(pane0.locator('[data-role="assistant"]')).toHaveCount(0)
  })
})
