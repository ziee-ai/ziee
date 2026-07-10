import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — keyboard acts on the pane you're typing in (TEST-59, ITEM-39).
 * The shared `document` keydown listener resolves its target through
 * `focusedPaneRoot()` (the focused pane's `[data-testid="chat-pane-<idx>"]`), NOT
 * `document.querySelector` first-match (which was always the leftmost pane). So
 * Ctrl+Enter typed in the SECOND (non-leftmost) pane sends THERE, leaving the
 * first pane idle. Uses the local OpenAI-compatible bridge.
 */
test.describe('Split chat — focused-pane keyboard (Ctrl+Enter)', () => {
  test('Ctrl+Enter sends in the pane you are typing in, not the leftmost pane', async ({
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
      data: { title: 'Keyboard Alpha' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // [A | new-chat]: pane 0 is the leftmost (conv A), pane 1 is a fresh new-chat.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Focus the SECOND pane, type, Ctrl+Enter → the send lands in pane 1.
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputB.click()
    await inputB.fill('Reply with exactly the single word: PONG')
    await inputB.press('Control+Enter')

    await expect(pane1.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    // The leftmost pane (pane 0) got nothing — the shortcut was focus-scoped, not
    // resolved by DOM order (the `document.querySelector` first-match bug).
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(0)
    await expect(pane0.locator('[data-role="assistant"]')).toHaveCount(0)
  })
})
