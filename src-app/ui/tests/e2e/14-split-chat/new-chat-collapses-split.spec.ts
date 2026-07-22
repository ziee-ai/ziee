import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — the sidebar's "New Chat" collapses the split workspace.
 *
 * Reported repro, reproduced LITERALLY (B9): open two conversations as a
 * left/right split, click "New Chat", type a message → instead of the new single
 * conversation, the view snapped back to the old 2-pane split with the new chat
 * jammed into one of the old panes.
 *
 * Mechanism: `ConversationPage` forks purely on pane count, so with 2 panes still
 * in `Stores.SplitView` the URL→workspace reconcile ran its "auto while split"
 * branch and REPLACED the focused pane with the newly created conversation. The
 * "New Chat" action is a plain navigate to `/chat`, and `NewChatPage` reset only
 * `Stores.Chat` — never the workspace. The fix resets `Stores.SplitView` on that
 * route, stating the invariant that `/chat` is a single-pane surface.
 *
 * The PAIRED control is `new-chat-adopt.spec.ts`: creating a chat from INSIDE a
 * split pane must still adopt into that pane and keep the split. Without it,
 * this spec could be satisfied by collapsing the split on every new-chat path,
 * destroying the in-pane flow. The two specs pin opposite sides of the boundary,
 * so they are meaningful only together.
 */
test.describe('Split chat — "New Chat" collapses the split (no hijack)', () => {
  test('TEST-8: 2-pane split → New Chat → send lands on a SINGLE pane showing the new conversation', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // A real provider + model so the composer can actually send (same bridge
    // setup the sibling adopt spec uses).
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const mkConv = async (title: string): Promise<string> => {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title },
      })
      expect(res.status()).toBeLessThan(300)
      return (await res.json()).id as string
    }
    const convA = await mkConv('Collapse Primary Alpha')
    const convB = await mkConv('Collapse Second Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // ── Build the reported starting state: TWO conversations, side by side.
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)
    // Precondition, asserted rather than assumed: a genuine 2-pane split. If
    // this is not reached, everything below would pass for the wrong reason.
    await expect(byTestId(page, 'split-chat-view')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
    await expect(pane1).toBeVisible()

    // ── The reported action: click "New Chat" in the sidebar.
    await byTestId(page, 'layout-sidebar-primary-actions-menu-item-new-chat').click()

    // We are on the new-chat route. NOTE: asserting `split-chat-view` is absent
    // HERE would prove nothing — `/chat` renders NewChatPage, and the split DOM
    // only ever exists under ConversationPage, so it is absent with or without
    // the fix. The store still holds two panes at this moment and that is not
    // observable from the DOM; the discriminating assertions are after the send,
    // once ConversationPage mounts again and acts on the pane count.
    await expect(byTestId(page, 'new-chat-greeting')).toBeVisible({ timeout: 15000 })

    // ── ...and type a message, which is where the old split used to reappear.
    const input = page.locator('textarea[placeholder*="Type your message"]')
    await input.click()
    await input.fill('Reply with exactly the single word: PONG')
    const send = page.getByRole('button', { name: 'Send message' })
    await expect(send).toBeEnabled({ timeout: 30000 })
    await send.click()

    // The message lands in a SINGLE-pane view of the NEW conversation.
    await expect(page.locator('[data-role="user"]')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)
    await expect(byTestId(page, 'chat-pane-1')).toHaveCount(0)

    // It really is a NEW conversation, not one of the two that were open —
    // this is the "jammed into one of the old panes" symptom, stated positively.
    await page.waitForURL(/\/chat\/[0-9a-f-]{36}$/i, { timeout: 30000 })
    const newId = page.url().split('/').pop()!
    expect(newId).not.toBe(convA)
    expect(newId).not.toBe(convB)

    // Exactly ONE conversation surface is rendered, holding exactly the message
    // just sent. (Deliberately not a page-wide "does not contain Alpha/Bravo"
    // text check — the sidebar's recent-chats list legitimately lists both
    // titles, so that assertion would fail for a reason unrelated to the fix.)
    await expect(page.locator('[data-role="user"]')).toHaveCount(1)
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toHaveCount(1)
  })
})
