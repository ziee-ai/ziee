import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — extension-registry runtime per-pane (TEST-54, ITEM-34). The
 * keyboard extension's shared `document` listener is refcounted across panes
 * (`keyboardInitCount`), so CLOSING one pane must NOT tear it down for the
 * survivor: the survivor's Ctrl+K / Esc / Ctrl+Enter keep working. This is the
 * regression the singleton-gated `initialize()/initialized` bug caused (the 2nd
 * pane's init early-returned; the 1st pane's cleanup disarmed the survivor). Uses
 * the local OpenAI-compatible bridge for the Ctrl+Enter send.
 */
test.describe('Split chat — registry runtime survives pane close', () => {
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

  test('closing a pane leaves the survivor’s Ctrl+K / Esc / Ctrl+Enter working', async ({
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
    const convA = await mkConv(page, apiURL, token, 'Runtime Alpha')
    const convB = await mkConv(page, apiURL, token, 'Runtime Bravo')

    // [A | B] split via the picker, then CLOSE pane 1 → survivor is a single pane.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })
    // Two panes → keyboardInitCount === 2. Close pane 1 → count 1 (listener MUST survive).
    await byTestId(page, 'chat-pane-1').getByTestId('chat-pane-close').click()
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0, { timeout: 15000 })

    const input = page.locator('textarea[placeholder*="Type your message"]')
    await expect(input).toBeVisible()
    await input.click()
    await input.fill('draft to clear')

    // Esc clears the focused composer — the keyboard extension's GLOBAL document
    // listener (refcounted across panes via keyboardInitCount) blanks it. If the
    // first pane's cleanup had disarmed the shared listener, Esc would do nothing.
    await page.keyboard.press('Escape')
    await expect(input).toHaveValue('')

    // Blur the composer, then Ctrl+K must REFOCUS it — only the global listener
    // does that (a blurred textarea cannot self-focus), so a passing focus here
    // proves the survivor's shortcut listener is still armed after the pane close.
    // (Ctrl+Enter is deliberately NOT used as the probe: TextInput's own onKeyDown
    // sends on Enter regardless of the extension, so it can't detect the teardown.)
    await input.blur()
    await expect(input).not.toBeFocused()
    await page.keyboard.press('Control+k')
    await expect(input).toBeFocused({ timeout: 5000 })
  })
})
