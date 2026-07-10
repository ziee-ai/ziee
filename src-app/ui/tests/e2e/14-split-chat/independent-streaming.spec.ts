import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — streaming routes to the CORRECT pane (TEST-15 / ITEM-2/6/10).
 *
 * Real streaming via the local OpenAI-compatible bridge (OPENAI_BASE_URL +
 * OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL point at it). Sending in pane A streams
 * the assistant reply into pane A ONLY; pane B (a fresh new-chat pane) stays idle
 * — proving the per-pane stream client + the applyStreamFrame conversation guard
 * keep the two live generations from cross-contaminating.
 *
 * Real send via the local OpenAI-compatible bridge.
 */
test.describe('Split chat — streaming isolation', () => {
  test('sending in pane A streams the reply into pane A only; pane B stays idle', async ({
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
      data: { title: 'Stream Pane A' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Open the split: pane 0 = conv A, pane 1 = a fresh new-chat pane.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // pane 1 is a new-chat pane (greeting), and has NO assistant message.
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Send a short prompt in pane 0. Click into the pane first (a real user
    // focuses the pane they type in — this sets the focused pane so the send
    // reads THIS pane's composer, matching the bridge/focus-on-interact model).
    const inputA = pane0.locator('textarea[placeholder*="Type your message"]')
    await expect(inputA).toBeVisible({ timeout: 15000 })
    await inputA.click()
    await inputA.fill('Reply with exactly the single word: PONG')
    const sendA = pane0.getByRole('button', { name: 'Send message' })
    await expect(sendA).toBeEnabled({ timeout: 30000 })
    await sendA.click()

    // The assistant reply streams into pane 0.
    await expect(pane0.locator('[data-role="assistant"]')).toBeVisible({
      timeout: 60000,
    })

    // ...and pane 1 NEVER receives an assistant message (no cross-pane bleed).
    await expect(pane1.locator('[data-role="assistant"]')).toHaveCount(0)
    // pane 1 is still the fresh new-chat pane.
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()
  })
})
