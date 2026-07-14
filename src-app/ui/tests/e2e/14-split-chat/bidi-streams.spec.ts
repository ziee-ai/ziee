import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — TRUE bidirectional streaming isolation under TWO SIMULTANEOUS
 * streams (TEST-98, audit #11). The existing `independent-streaming.spec` used an
 * IDLE empty pane as the "other pane unaffected" control, so it only proved "A
 * doesn't leak into a quiescent B". This drives BOTH panes streaming at once and
 * asserts each direction: pane A ends with ITS prompt + one assistant reply and
 * never shows B's content, and vice-versa. Real streaming via the local bridge.
 */
test.describe('Split chat — two simultaneous streams, bidirectional isolation', () => {
  test('both panes stream at once; each keeps its own user+assistant, neither shows the other', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(150000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('Bidi Alpha')
    const convB = await mkConv('Bidi Bravo')

    // [A | B] split, both holding a real conversation (no idle control).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })

    const fill = async (pane: ReturnType<typeof byTestId>, prompt: string) => {
      const ta = pane.locator('textarea[placeholder*="Type your message"]')
      await ta.click()
      await ta.fill(prompt)
    }
    // Distinct, unique tokens each pane's USER message carries (deterministic —
    // we control the prompt text; the assistant text is not relied upon).
    await fill(pane0, 'Please reply. Marker ALPHATOKEN.')
    await fill(pane1, 'Please reply. Marker BRAVOTOKEN.')

    // Fire BOTH sends back-to-back so the two streams are in flight together.
    await pane0.getByRole('button', { name: 'Send message' }).click()
    await pane1.getByRole('button', { name: 'Send message' }).click()

    // Both panes stream a reply concurrently — each gets its OWN assistant message.
    await expect(pane0.locator('[data-role="assistant"]')).toHaveCount(1, { timeout: 60000 })
    await expect(pane1.locator('[data-role="assistant"]')).toHaveCount(1, { timeout: 60000 })

    // Each pane kept exactly its OWN user message (the marker it was sent).
    await expect(pane0.locator('[data-role="user"]')).toContainText('ALPHATOKEN')
    await expect(pane1.locator('[data-role="user"]')).toContainText('BRAVOTOKEN')
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(1)
    await expect(pane1.locator('[data-role="user"]')).toHaveCount(1)

    // BIDIRECTIONAL: neither pane ever shows the OTHER pane's marker (no cross-bleed
    // of either the user prompt or the streamed reply, in either direction).
    await expect(pane0.getByText('BRAVOTOKEN')).toHaveCount(0)
    await expect(pane1.getByText('ALPHATOKEN')).toHaveCount(0)
  })
})
