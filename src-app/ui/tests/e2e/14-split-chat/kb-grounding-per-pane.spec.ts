import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane KB grounding (TEST-69, ITEM-46). Attaching a knowledge
 * base in one split pane's composer grounds THAT pane's conversation only; the
 * other pane's chips are unaffected (the KB composer selection is now
 * per-conversation, not a single global). No LLM.
 */
test.describe('Split chat — per-pane KB grounding', () => {
  test.describe.configure({ retries: 1 })

  test('attaching a KB in pane B grounds pane B only; pane A is unaffected', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const mkConv = async (title: string) =>
      (
        await (
          await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title } })
        ).json()
      ).id as string
    const convA = await mkConv('KB Ground A')
    const convB = await mkConv('KB Ground B')
    const kb = await (
      await page.request.post(`${apiURL}/api/knowledge-bases`, {
        headers: auth,
        data: { name: 'Ground KB' },
      })
    ).json()

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.getByTestId('conversation-picker-pane')).toHaveCount(0)

    // Attach the KB via pane B's + menu (the menu portals but carries pane B's
    // context, so it grounds pane B's conversation).
    await pane1.getByTestId('chat-input-add-btn').click()
    await byTestId(page, 'kb-menu-trigger').click()
    await byTestId(page, `kb-option-${kb.id}`).click()

    // The KB chip shows in pane B ONLY, never pane A.
    await expect(pane1.getByTestId(`kb-chip-${kb.id}`)).toBeVisible({ timeout: 15000 })
    await expect(pane0.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)

    // Focusing pane A does not surface B's KB (the chip is per-conversation, not
    // focus-following) — pane A's own grounding stays empty.
    await pane0.click({ position: { x: 200, y: 80 } })
    await expect(pane0.getByTestId(`kb-chip-${kb.id}`)).toHaveCount(0)
    await expect(pane1.getByTestId(`kb-chip-${kb.id}`)).toBeVisible()
  })
})
