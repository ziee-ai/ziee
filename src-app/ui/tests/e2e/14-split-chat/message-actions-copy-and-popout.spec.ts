import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — the message-action legs NOT covered by
 * `message-actions-per-pane.spec.ts` (TEST-58, which drives regenerate / edit /
 * branch). Closes the audit-surfaced gap:
 *
 * - TEST-86 (copy per-pane): the Copy button reads the message it is RENDERED on
 *   (`useMessageContext()`), never a global focused-message. Copying pane B's
 *   message while pane A is focused puts pane B's text on the clipboard.
 * - TEST-87 (pop-out window): the pop-out (`/chat-window/:id`) has no pane
 *   provider, so message actions bind to the global `Stores.Chat` scoped to that
 *   window's own conversation — the full action set (copy/edit/regenerate)
 *   renders and functions there.
 *
 * Real send via the local OpenAI-compatible bridge.
 */
test.describe('Split chat — copy per-pane + pop-out-window message actions', () => {
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

  test('Copy binds to the render-pane message; pop-out window keeps the full action set', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(180000)
    const { baseURL, apiURL } = testInfra
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const convA = await mkConv(page, apiURL, token, 'CopyAudit Alpha')
    const convB = await mkConv(page, apiURL, token, 'CopyAudit Bravo')

    // [A | B] split via the picker; send one turn in pane 1 (B).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()

    const MSG = 'Reply with exactly the single word: MANGO'
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputB.click()
    await inputB.fill(MSG)
    await pane1.getByRole('button', { name: 'Send message' }).click()
    await expect(pane1.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(pane1.locator('[data-role="assistant"]')).toBeVisible({ timeout: 60000 })

    // TEST-86 — focus pane 0 (empty), then COPY pane 1's user message. The
    // clipboard must carry PANE 1's text (the button reads its own message), and
    // pane 0 must still hold no message (isolation).
    await pane0.click()
    await expect(pane0).toHaveClass(/opacity-100/)
    await expect(pane0.locator('[data-role="user"]')).toHaveCount(0)

    const userMsgB = pane1.locator('[data-role="user"]').first()
    await userMsgB.hover()
    const copyBtnB = pane1.getByTestId('chat-message-copy-btn').first()
    await expect(copyBtnB).toBeVisible()
    await copyBtnB.click()
    await expect(async () => {
      const clip = await page.evaluate(() => navigator.clipboard.readText())
      expect(clip).toBe(MSG)
    }).toPass({ timeout: 10000 })

    // TEST-87 — the pop-out window (no pane provider) for convB shows the FULL
    // message-action set and edit populates its composer. Clear any split state so
    // the layout-less window renders bare.
    await page.evaluate(() =>
      Object.keys(localStorage)
        .filter((k) => k.toLowerCase().includes('split'))
        .forEach((k) => localStorage.removeItem(k)),
    )
    await page.goto(`${baseURL}/chat-window/${convB}`)
    await expect(page.locator('[data-role="assistant"]').first()).toBeVisible({ timeout: 20000 })
    const popUser = page.locator('[data-role="user"]').first()
    await popUser.hover()
    // copy on BOTH messages (=2), edit on the user message (=1), regen on the
    // assistant message (=1) — the same set single-pane renders.
    await expect(byTestId(page, 'chat-message-copy-btn')).toHaveCount(2)
    await expect(byTestId(page, 'edit-message-button')).toHaveCount(1)
    await expect(byTestId(page, 'regenerate-button')).toHaveCount(1)

    // Edit works in the pop-out: the message text restores into the window's composer.
    await byTestId(page, 'edit-message-button').first().click()
    await expect(page.locator('textarea[placeholder*="Type your message"]').first()).toHaveValue(
      /MANGO/,
      { timeout: 15000 },
    )
  })
})
