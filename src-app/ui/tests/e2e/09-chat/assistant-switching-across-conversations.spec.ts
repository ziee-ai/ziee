import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — per-conversation assistant-picker reset on conversation switch
 * (audit all-23d59c7f31b8).
 *
 * `AssistantPicker.store.ts`'s `selectedAssistantId` is scoped to the
 * ACTIVE conversation: the assistant chat-extension subscribes to
 * `state.conversation?.id` and calls `Stores.AssistantPicker.reset()`
 * whenever it changes (extension.tsx:41-46). No E2E exercised this.
 *
 * The proof hinges on CLIENT-SIDE navigation: switching conversations
 * via the sidebar (react-router `navigate`, no document reload) keeps
 * the picker store alive, so the chip can only clear because the
 * reset-on-conversation-change subscriber fired — not because a full
 * page reload threw the store away. Deterministic, no LLM.
 */

test.describe('Chat — assistant picker resets across conversation switches', () => {
  test('selecting an assistant in one conversation is cleared after switching to another (and not restored on return)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const tag = Date.now().toString(36)
    const assistantName = `Switch Assistant ${tag}`

    // A distinctively-named assistant so the picker submenu + status chip
    // are unambiguous.
    const created = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ name: assistantName, instructions: 'Be terse.' }),
    })
    expect(created.status).toBeLessThan(300)

    // Two real conversations to switch between (unfiled → both surface in
    // the sidebar Recent list).
    const titleA = `ZZZ Conv A ${tag}`
    const titleB = `ZZZ Conv B ${tag}`
    const mkConv = async (title: string): Promise<string> => {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title },
      })
      expect(res.status()).toBeLessThan(300)
      return (await res.json()).id as string
    }
    const convA = await mkConv(titleA)
    const convB = await mkConv(titleB)

    // Land in conversation A (single full navigation — every switch AFTER
    // this is client-side so the picker store survives).
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    const addBtn = byTestId(page, 'chat-input-add-btn')
    await expect(addBtn).toBeVisible({ timeout: 30000 })

    const chip = () =>
      byTestId(page, 'assistant-status-chip')

    // --- Select the assistant in conversation A → its status chip shows. ---
    await addBtn.click()
    await byTestId(page, 'assistant-menu-trigger').click()
    await expect(page.getByText(assistantName)).toBeVisible({ timeout: 10000 })
    await page.getByText(assistantName).click()
    await expect(chip()).toBeVisible({ timeout: 10000 })

    // Both conversations are reachable in the sidebar (client-side nav). The
    // recent-conversations Menu derives one item per conversation id.
    const rowA = byTestId(page, `chat-recent-conversations-menu-item-${convA}`)
    const rowB = byTestId(page, `chat-recent-conversations-menu-item-${convB}`)
    await expect(rowB).toBeVisible({ timeout: 15000 })

    // --- Switch to conversation B via the sidebar (SPA navigation). ---
    // conversation.id changes A→B → the extension's subscriber fires
    // reset() → the picker selection clears. A regression that scoped the
    // selection globally (or dropped the subscriber) would leave the chip.
    await rowB.click()
    await expect(page).toHaveURL(new RegExp(`/chat/${convB}`), { timeout: 15000 })
    await expect(chip()).toHaveCount(0, { timeout: 10000 })

    // --- Switch back to conversation A (SPA navigation). ---
    // reset() fires again; the selection is NOT restored (the store holds a
    // single active-conversation value, not a per-conversation map), so A's
    // chip stays cleared — proving the reset is on every conversation change.
    await rowA.click()
    await expect(page).toHaveURL(new RegExp(`/chat/${convA}`), { timeout: 15000 })
    await expect(chip()).toHaveCount(0, { timeout: 10000 })
  })
})
