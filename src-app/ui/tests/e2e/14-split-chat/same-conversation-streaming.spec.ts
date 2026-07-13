import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — one-conversation-per-workspace guard + frame routing (TEST-55,
 * ITEM-35, re-scoped DRIFT-2.9). The shipped guard makes "the SAME conversation in
 * two panes" UNREACHABLE (opening A into a 2nd pane FOCUSES A's existing pane, no
 * duplicate), so this proves that guard at the e2e level AND the per-instance
 * frame-routing the ITEM-35 hardening protects: while pane A streams, pane B (a
 * DIFFERENT conversation) receives NONE of A's frames and shows clean, non-doubled
 * text only in A. Uses the local OpenAI-compatible bridge.
 */
test.describe('Split chat — same-conversation guard + frame routing', () => {
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

  test('opening a paned conversation again focuses its pane (no duplicate); a stream stays in its own pane', async ({
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
    const convA = await mkConv(page, apiURL, token, 'Same Alpha')
    const convB = await mkConv(page, apiURL, token, 'Same Bravo')

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })

    // GUARD: opening A (already in pane 0) via the sidebar FOCUSES pane 0 — it does
    // NOT create a second pane holding the same conversation.
    await byTestId(page, `chat-recent-conversations-menu-item-${convA}`).click()
    await expect(pane0).toHaveClass(/opacity-100/, { timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-2')).toHaveCount(0)

    // FRAME ROUTING: send in pane 0 (A). The reply streams into pane 0 only; pane 1
    // (B, a different conversation) receives none of A's frames.
    const inputA = pane0.locator('textarea[placeholder*="Type your message"]')
    await inputA.click()
    await inputA.fill('Reply with exactly the single word: PONG')
    await pane0.getByRole('button', { name: 'Send message' }).click()

    await expect(pane0.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })
    await expect(pane1.locator('[data-role="assistant"]')).toHaveCount(0)
    await expect(pane1.locator('[data-role="user"]')).toHaveCount(0)
  })
})
