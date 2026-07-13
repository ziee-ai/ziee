import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — new-chat pane adopts its created conversation IN PLACE
 * (TEST-37 / TEST-38 / TEST-45). Sending in a second (new-chat) pane creates a
 * conversation and adopts it into THAT pane's SplitView slot: (a) it does NOT
 * navigate the whole window away (no `conversation.created` window-hijack,
 * TEST-37); (b) the pane opens a fresh working stream for the new conversation
 * (TEST-38); (c) the other pane is left completely undisturbed — the in-pane
 * conversation change is scoped to one pane (TEST-45 / DEC-14 in-pane switch).
 * Real send via the local OpenAI-compatible bridge.
 */
test.describe('Split chat — new-chat pane adopt (no window hijack)', () => {
  test('sending in the new-chat pane adopts the conversation in-place; the window + other pane are untouched', async ({
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
      data: { title: 'Adopt Primary A' },
    })
    const convA = (await res.json()).id as string
    const primaryUrl = `${baseURL}/chat/${convA}`

    await page.goto(primaryUrl)
    await page.waitForLoadState('load')

    // Split: pane 0 = conv A, pane 1 = a fresh new-chat pane (greeting shown).
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    // pane 1 opens the conversation PICKER (ITEM-27); "Start a new chat" reaches
    // the new-chat composer that adopts the created conversation into this slot.
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Send in the new-chat pane.
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputB.click()
    await inputB.fill('Reply with exactly the single word: PONG')
    const sendB = pane1.getByRole('button', { name: 'Send message' })
    await expect(sendB).toBeEnabled({ timeout: 30000 })
    await sendB.click()

    // (b) TEST-38: the pane opened a fresh working stream — a user message + a
    // streamed assistant reply appear in pane 1.
    await expect(pane1.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(pane1.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })
    // The new-chat greeting is gone — the pane adopted the conversation in place.
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toHaveCount(0)

    // (a) TEST-37: the WINDOW did not navigate AWAY (no `conversation.created`
    // window-hijack) — the SPLIT is intact, both panes still present. Under the
    // URL-tracks-focused-pane model (ITEM-72), the address bar DOES follow the
    // focused pane (pane 1) to its newly-adopted conversation — that is correct, not
    // a hijack (the split is preserved), so the "no hijack" proof is the surviving
    // split + the undisturbed other pane below, NOT a frozen URL.
    await expect(byTestId(page, 'split-chat-view')).toBeVisible()
    await expect(pane0).toBeVisible()
    await expect(pane1).toBeVisible()
    expect(page.url()).not.toBe(primaryUrl) // it tracks the focused (adopting) pane now

    // (c) TEST-45: pane 0 (conv A) is completely undisturbed — no messages leaked
    // in from pane 1's new conversation.
    await expect(pane0.locator('[data-role="assistant"]')).toHaveCount(0)
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(0)
    await expect(pane0).toBeVisible()
  })
})
